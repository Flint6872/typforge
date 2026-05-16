#set page(
  paper: "us-letter",
  margin: (x: 1in, y: 1in),
  header: align(left)[Typst Document],
  footer:  none,
)

#set text(size: 11pt)
#set par(justify: true, leading: 0.6em)
#set heading(numbering: "1.1.")

// --- Title Page ---
  #align(center + horizon)[
  #text(size: 2em, weight: "bold")[My 5-Page Document]
  #v(1em)
  #text(size: 1.2em)[Author Name]
  #v(2em)
  #datetime.today().display()
  #show link: underline

https://example.com \

#link("https://example.com") \
#link("https://example.com")[
  See example.com
]
]
#pagebreak()

// --- Table of Contents ---
#outline(indent: auto)
#pagebreak()

// --- Content --
#set page(footer: context align(right, counter(page).display()))
#counter(page).update(1)

= Introduction
#lorem(100)

#pagebreak()

= Methodology
#lorem(150)


== Data Collection
#lorem(100)

=== Analysis Techniques
#lorem(120)

#pagebreak() // Force start on new page
= Results
#lorem(200)

#table(
  columns: (auto, 1fr, 1fr),
  rows: auto,
  [*ID*], [*Metric A*], [*Metric B*],
  [1], [#lorem(10)], [#lorem(10)],
  [2], [#lorem(10)], [#lorem(10)],
)
#lorem(100)

#pagebreak()
= Discussion
#lorem(200)

= Conclusion
#lorem(100)

#pagebreak()
= References
#lorem(50)
