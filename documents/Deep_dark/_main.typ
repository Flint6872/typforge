#set page(width: 5.5in, height: 8.5in)

#set text(
  font: "Times New Roman",
  size: 11pt,
  fill: rgb("#111111"),
  lang: "en",
)

// --- Table of Contents ---
#outline(indent: auto)
#pagebreak()

#set page(footer: context align(right, counter(page).display()))
#counter(page).update(1)
#include "chapter-1.typ"
#pagebreak()

#include "chapter-2.typ"
#pagebreak()

#include "chapter-3.typ"
#pagebreak()

#include "chapter-4.typ"
#pagebreak()

#include "chapter-5.typ"
#pagebreak()

#include "chapter-6.typ"
#pagebreak()

#include "chapter-7.typ"
#pagebreak()

#include "chapter-8.typ"
#pagebreak()

#include "chapter-9.typ"
