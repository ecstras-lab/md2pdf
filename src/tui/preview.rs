//! The page, laid out and drawn away from the interface.
//!
//! Three things happen off the main thread, so that the keyboard is always
//! answered at once. A note is typeset and rendered to pixels. A slice of those
//! pixels is encoded for the terminal every time the view moves. And a finished
//! page is written to a PDF. The main thread only ever draws the latest result
//! each of these hands back.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use image::{DynamicImage, RgbaImage, imageops};
use ratatui::layout::Size;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use ratatui_image::{FilterType, Resize};
use typst_layout::PagedDocument;

use crate::cli::ThemeName;
use crate::convert;
use crate::document::compile;

/// The page width the stylesheet sets, in points. The render is scaled so that
/// this maps onto the width of the pane, whatever the pane happens to be.
const PAGE_WIDTH: f64 = 600.0;

/// The page is drawn at this many times the pane resolution, then shrunk to the
/// pane. Downsampling from a larger render is what makes the small text crisp
/// rather than the soft result of rendering straight at terminal resolution.
const SUPERSAMPLE: u32 = 2;

/// A ceiling on the rendered height, in pixels. A long note at full supersample
/// could otherwise ask for an image too tall to allocate, so past this the
/// render gives up some sharpness to stay within it.
const MAX_HEIGHT: u32 = 16000;

/// One note, one theme, one pane. Everything a render needs.
pub(super) struct Request {
    pub(super) token: u64,
    pub(super) path: PathBuf,
    pub(super) theme: ThemeName,
    pub(super) pane: Size,
    pub(super) font: Size,
}

/// A rendered note. The laid out document rides along, so that writing the PDF
/// exports the very pages the reader was looking at rather than a fresh compile
/// that might disagree. The theme rides along so the interface can tell whether
/// the page it holds still matches the theme it is now showing.
pub(super) struct Page {
    pub(super) document: PagedDocument,
    pub(super) image: Arc<RgbaImage>,
    pub(super) warnings: Vec<String>,
    pub(super) theme: ThemeName,
}

/// A written PDF.
pub(super) struct Export {
    pub(super) path: PathBuf,
    pub(super) bytes: usize,
    pub(super) elapsed: Duration,
}

/// Something a worker finished. Typesetting and writing are both slow and both
/// rare, so they share one channel back to the interface.
pub(super) enum Done {
    Typeset {
        token: u64,
        page: Result<Page, String>,
    },
    Export(Result<Export, String>),
}

/// A view to encode: a slice of a page's image, named by a generation so a
/// drawing for a scroll position the reader has already left can be ignored.
pub(super) struct EncodeRequest {
    pub(super) generation: u64,
    pub(super) image: Arc<RgbaImage>,
    pub(super) pane: Size,
    pub(super) font: Size,
    pub(super) scroll: u32,
}

pub(super) struct EncodeReply {
    pub(super) generation: u64,
    pub(super) protocol: Option<Protocol>,
}

/// Typesets and renders a note, off the main thread.
pub(super) fn typeset_in_background(
    request: Request,
    done: Sender<Done>,
) {
    thread::spawn(move || {
        let token = request.token;
        let page = render(request).map_err(|error| error.to_string());

        // The receiver is gone only once the interface has closed.
        let _ = done.send(Done::Typeset { token, page });
    });
}

/// Writes a finished document to a PDF, off the main thread.
pub(super) fn export_in_background(
    document: PagedDocument,
    output_path: PathBuf,
    done: Sender<Done>,
) {
    thread::spawn(move || {
        let _ = done.send(Done::Export(write_pdf(&document, output_path)));
    });
}

/// Spawns the lone encoder, which turns whichever view is latest into something
/// the terminal can draw. It owns its own copy of the picker, and lives as long
/// as the interface holds the sending end.
pub(super) fn spawn_encoder(picker: Picker) -> (Sender<EncodeRequest>, Receiver<EncodeReply>) {
    let (request_tx, request_rx) = mpsc::channel::<EncodeRequest>();
    let (reply_tx, reply_rx) = mpsc::channel();

    thread::spawn(move || {
        while let Ok(mut request) = request_rx.recv() {
            // A burst of scrolls leaves several views waiting. Only the last is
            // worth drawing, so the encoder skips to it.
            while let Ok(newer) = request_rx.try_recv() {
                request = newer;
            }

            let protocol = encode(
                &picker,
                &request.image,
                request.pane,
                request.font,
                request.scroll,
            )
            .ok();

            let reply = EncodeReply {
                generation: request.generation,
                protocol,
            };

            if reply_tx.send(reply).is_err() {
                break;
            }
        }
    });

    (request_tx, reply_rx)
}

fn render(request: Request) -> Result<Page> {
    let typeset = convert::prepare(&request.path, &request.theme.build())?.typeset()?;

    let scale = render_scale(&typeset.document, request.pane, request.font);
    let image = compile::to_image(&typeset.document, scale)?;

    Ok(Page {
        document: typeset.document,
        image: Arc::new(image),
        warnings: typeset.warnings,
        theme: request.theme,
    })
}

/// Pixels per point for the render, at supersample, but never so many that the
/// image would stand taller than the ceiling.
fn render_scale(
    document: &PagedDocument,
    pane: Size,
    font: Size,
) -> f64 {
    let supersampled = f64::from(image_pixels(pane.width, font.width)) / PAGE_WIDTH;

    let page_height = document
        .pages()
        .first()
        .map(|page| page.frame.size().y.to_pt())
        .unwrap_or(PAGE_WIDTH)
        .max(1.0);

    let ceiling = f64::from(MAX_HEIGHT) / page_height;

    supersampled.min(ceiling)
}

fn write_pdf(
    document: &PagedDocument,
    output_path: PathBuf,
) -> Result<Export, String> {
    let started = Instant::now();

    let pdf = compile::to_pdf(document).map_err(|error| error.to_string())?;
    let bytes = pdf.len();

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    std::fs::write(&output_path, pdf).map_err(|error| error.to_string())?;

    Ok(Export {
        path: output_path,
        bytes,
        elapsed: started.elapsed(),
    })
}

/// The slice of the page that fills the pane, shrunk from the supersampled
/// render to the pane's own resolution, and encoded for the terminal.
fn encode(
    picker: &Picker,
    image: &RgbaImage,
    pane: Size,
    font: Size,
    scroll: u32,
) -> Result<Protocol> {
    let screen = image_pixels(pane.height, font.height);

    let top = scroll.min(image.height().saturating_sub(1));
    let visible = screen.min(image.height() - top);

    if image.width() == 0 || visible == 0 {
        return Err(anyhow!("there is no room to draw the page"));
    }

    let crop = imageops::crop_imm(image, 0, top, image.width(), visible).to_image();

    picker
        .new_protocol(
            DynamicImage::ImageRgba8(crop),
            pane,
            Resize::Fit(Some(FilterType::Lanczos3)),
        )
        .map_err(|error| anyhow!("the page could not be drawn\n{error}"))
}

/// How far the page can be pushed up before its last row reaches the top of the
/// pane. A page shorter than the pane does not move.
pub(super) fn furthest_scroll(
    image: &RgbaImage,
    pane: Size,
    font: Size,
) -> u32 {
    image
        .height()
        .saturating_sub(image_pixels(pane.height, font.height))
}

/// One text row, in image pixels. Scrolling moves by whole rows of these.
pub(super) fn line_height(font: Size) -> u32 {
    image_pixels(1, font.height)
}

/// A count of cells, in image pixels, which is device pixels times the
/// supersample the page is rendered at.
fn image_pixels(
    cells: u16,
    per_cell: u16,
) -> u32 {
    u32::from(cells) * u32::from(per_cell) * SUPERSAMPLE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(
        width: u32,
        height: u32,
    ) -> RgbaImage {
        RgbaImage::new(width, height)
    }

    #[test]
    fn a_page_taller_than_the_pane_can_be_scrolled_to_its_last_row() {
        let pane = Size::new(60, 40);
        let font = Size::new(8, 16);

        let furthest = furthest_scroll(&page(480, 5000), pane, font);

        assert_eq!(furthest, 5000 - 40 * 16 * SUPERSAMPLE);
    }

    #[test]
    fn a_page_that_fits_the_pane_does_not_scroll() {
        let pane = Size::new(60, 40);
        let font = Size::new(8, 16);
        let screen = 40 * 16 * SUPERSAMPLE;

        assert_eq!(furthest_scroll(&page(480, screen - 1), pane, font), 0);
        assert_eq!(furthest_scroll(&page(480, screen), pane, font), 0);
    }

    /// Halfblocks report a font size too, so the pane is measured the same way
    /// whichever protocol the terminal turned out to support.
    #[test]
    fn a_pane_is_measured_in_pixels_by_its_font() {
        assert_eq!(image_pixels(60, 8), 60 * 8 * SUPERSAMPLE);
        assert_eq!(image_pixels(0, 8), 0);
        assert_eq!(line_height(Size::new(8, 16)), 16 * SUPERSAMPLE);
    }
}
