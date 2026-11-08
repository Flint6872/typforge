#set page(paper: "a4", margin: 2.5cm)
#set text(size: 11pt)

= Project Introduction

As famously stated in physics history, early foundational work laid out the laws of motion @netwon1687.

Decades later, computing paradigms shifted when the foundations of artificial intelligence were formally questioned by @turing1950.

For modern technical writing, we lean on open-source typesetting systems like the native bibliography engine detailed by the @typst2026.

// 1. Render the bibliography at the bottom of the page
// 2. Control the output style (e.g., "apa", "ieee", "mla")
#bibliography("works.bib", style: "apa")
