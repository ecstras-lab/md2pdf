---
title: Obsidian Markdown Test File
tags:
  - test
  - markdown
  - obsidian
created: 2026-03-26
aliases:
  - MD Test
  - Obsidian Demo
Tickbox: true
---

# H1 Heading

## H2 Heading

### H3 Heading

#### H4 Heading

##### H5 Heading

###### H6 Heading

---

## Text Formatting

**Bold text**  
*Italic text*  
***Bold + Italic***  
~~Strikethrough~~  
==Highlight long==

### Paragraphs

This is a story about a man named Jack. He went up the hill. Then he rolled over and fell down the hill. He hit his head and died.

This is the second paragraph. Sad story?

---

## Lists

### Unordered

- Item 1
- Item 2
  - Nested Item
    - Deep Nested

### Ordered
1. First
2. Second
	1. Sub-item
	2. Sub-item

### Task List

- [ ] Incomplete task
- [x] Completed task

---

## Links

[External Link](https://example.com)  

---

## Blockquotes

> This is a blockquote  
>> Nested quote  
>>> Deep nested quote

> Single quote

> Multi-Line
> Quote

---

## All Callouts

> [!note]
> note

> [!info]
> info

> [!important]
> important

> [!tip]
> tip

> [!success]
> success

> [!question]
> question

> [!warning]
> warning

> [!example]
> example

> [!quote]
> quote

> [!abstract]
> abstract

> [!summary]
> summary

> [!tldr]
> tldr

> [!todo]
> todo

> [!hint]
> hint

> [!check]
> check

> [!done]
> done

> [!faq]
> faq

> [!help]
> help

> [!caution]
> caution

> [!attention]
> attention

> [!failure]
> failure

> [!fail]
> fail

> [!missing]
> missing

> [!danger]
> danger

> [!error]
> error

> [!bug]
> bug

> [!cite]
> cite

> [!QUOTE] Single line blockquote

---

## Code

Inline code: `print("hello")`

### Code Block
```python
def hello():
    print("Hello, world")
```

### Code Block (No Language)

```
Some plain text
```

```
$This is not latex$
$$neither should this be$$

%%Should not be comment%%
==Should not higlight==
**Should not bold**
```

---

## Tables

| Column 1 | Column 2 | Column 3 |
| -------- | -------- | -------- |
| A        | B        | C        |
| 1        | 2        | 3        |

---

## Horizontal Rule

---

## Footnotes

Here is a sentence with a footnote[^1]
1. Here is another sentence with a ==footnote==[^2]. And text after.

---

## Math (LaTeX)

Inline: $E = mc^2$

Block:  
$$  
\int_0^1 x^2 dx  
$$

Should not be latex: $10 to $20

---

## Embeds

![[Another Note]]

---

## Tags

#tag  
#tag/miaow

---

## Comments

%% This is a comment %%

---

## Highlighted Block

==This is highlighted text==

---

## Escape Characters

*Should be italic*  
# Should be a heading

---

## Mixed Example

> [!note]  
> **Important:**
> 
> - Item 1
>     
> - Item 2
>     
> 
> ```js
> console.log("inside note");
> ```
> > Note: miaow

---

## Image

![[Pasted image 20260326170345.png]]

![A dummy banner, wider than the page](<Pasted image 20260326170345.png>)

![[not-here.png]]

---

## Video

![[2026-03-26 11-50-04.mp4]]

[^1]: This is the footnote content
[^2]: This is the footnote content
