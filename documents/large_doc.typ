#set page(
  paper: "a4",
  margin: (x: 2cm, y: 2.5cm),
  header: align(right)[
    #text(size: 8pt, fill: rgb("666666"))[TypForge Performance Stress Test Suite v0.1.0]
  ],
  footer: context [
    #set text(size: 9pt, fill: rgb("444444"))
    #grid(
      columns: (1fr, auto),
      [Confidential / Benchmark Build],
      [Page #counter(page).display()]
    )
  ]
)

#set text(
  font: "Liberation Sans",
  size: 11pt,
  fill: rgb("222222")
)

#set par(
  justify: true,
  leading: 0.75em,
  first-line-indent: 1.5em
)

#set block(above: 1.2em, below: 1.2em)

// --- TITLE & COVER PAGE ---
#align(center + horizon)[
  #text(size: 28pt, weight: "bold", fill: rgb("1a1a1a"))[TypForge Extreme Benchmark]

  #text(size: 14pt, style: "italic", fill: rgb("555555"))[A 1500-Page Stress Test for Typst & GPUI Rendering]

  #v(2em)
  #rect(width: 80%, stroke: 1pt + rgb("cccccc"), inset: 1.5em)[
    #set text(size: 10pt)
    #align(left)[
      *Test Parameters:*
      - Target Length: $approx 1500$ Pages
      - Elements: Grids, Tables, Linear Gradients, Math Blocks, Syntax Trees, and Columns.
      - Objective: Evaluate background async compilation, UI frame-rate stability, memory atlas recycling, and hit-test accuracy.
    ]
  ]
]

#pagebreak()

#outline(depth: 2, indent: 1.5em)
#pagebreak()

// --- HELPER MACROS ---
#let stress_block(id) = [
  #block(
    fill: rgb("f4f4f6"),
    inset: 12pt,
    radius: 4pt,
    stroke: (left: 4pt + rgb("4a90e2")),
    width: 100%
  )[
    #text(weight: "bold", fill: rgb("4a90e2"))[Benchmark Block Node \#id]

    The system is processing asynchronous compilation task vector \#id. Complex layout matrices require exact scale bounds calculation, gradient rasterization cache lookups, and glyph hit-mapping reconciliation across the viewport.

    - Primary CPU thread load: *Stable*
    - GPUI texture atlas allocation: *Optimized*
    - AST node traversal depth: *Recursive*
  ]
]

#let complex_table = [
  #table(
    columns: (1fr, 1.5fr, 1fr, 1fr),
    fill: (col, row) => if row == 0 { rgb("4a90e2") } else if calc.even(row) { rgb("f9f9f9") } else { none },
    stroke: 0.5pt + rgb("dddddd"),
    align: (col, row) => if row == 0 { center + horizon } else { left + horizon },

    table.header(
      [*ID*], [*Component Descriptor*], [*Status*], [*Latency (ms)*]
    ),

    [T-001], [GPUI Text System Atlas Integration], [Active], [0.42],
    [T-002], [Typst AST LinkedNode Parser], [Synchronized], [1.18],
    [T-003], [Background Raycast HitMap Collector], [Verified], [0.15],
    [T-004], [Gradient Rasterization CPU Fallback], [Cached], [2.40],
    [T-005], [Multi-Tab File State Buffer Manager], [Ready], [0.08],
  )
]

#let math_stress_section = [
  $ cal(H)_("total") = sum_(i=1)^N (-frac(h.bar^2, 2m_i) nabla_i^2 + V(r_i)) + integral_0^infinity vec(E)(x, t) times vec(B)(x, t) \ d V $
]

// --- DOCUMENT BODY GENERATION ---
// 300 iterations x 5 pages per iteration yields ~1500 total pages
#for chapter_num in range(1, 301) [
  = Chapter #chapter_num: Automated Stress Vector Analysis

  This section executes structured evaluation sequence #chapter_num. It embeds demanding visual components to test the limits of GPUI's quad painter and font registry sync.

  == Typography & Inline Styling

  Normal running text flows seamlessly around embedded items. Here is a sample of *bold emphasis*, _italic styling_, and #underline[underlined inline structures] combined with explicit font styling like #text(fill: rgb("d9534f"))[custom colored warnings] and #text(size: 13pt, weight: "bold")[large scaled inline headings] embedded directly inside paragraphs.

  == Structural Layout Matrices

  #complex_table

  == Mathematical Proofs & Tensor Formulations

  #math_stress_section

  == Multi-Column Subsection Layout

  #columns(2)[
    Left column block containing dense analytical data stream. Notice how the multi-column layout splits automatically without clipping glyph bounds or failing hit-test coordinate translations.

    #colbreak()

    Right column block containing supplementary execution notes. Background tasks continue to run asynchronously without dropping frames or freezing the active window thread.
  ]

  == Diagnostic Block Embeddings

  #grid(
    columns: (1fr, 1fr),
    gutter: 1em,
    stress_block(chapter_num * 2 - 1),
    stress_block(chapter_num * 2)
  )

  // Page padding within each chapter to reach ~5 pages per chapter
  #pagebreak()


  == Deep AST Traversal Sub-node [#chapter_num].A

    #lorem(180)

    #complex_table

    #lorem(120)

    #pagebreak()

    == Deep AST Traversal Sub-node [#chapter_num].B

    #math_stress_section

    #lorem(220)

    #grid(
      columns: (1fr, 1fr),
      gutter: 1em,
      stress_block(chapter_num * 10 + 1),
      stress_block(chapter_num * 10 + 2)
    )

    #pagebreak()

    == Deep AST Traversal Sub-node [#chapter_num].C

    #lorem(250)

    #complex_table

    #pagebreak()
]
