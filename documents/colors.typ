#set page(
  paper: "us-letter",
  margin: (x: 1in, y: 1in),
  header: none,
  footer: none,
)

#grid(
  columns: 2,
  rows: 2,
  column-gutter: 1em,
  row-gutter: 1em,

  [
    #text(red)[Red] \
    #text(orange)[Orange] \
    #text(rgb("#FFD700"))[Gold] \
    #text(yellow)[Yellow] \
    #text(lime)[Lime]\
  ],
  [
    #text(green)[Green] \
    #text(blue)[Blue] \
    #text(rgb("#4B0082"))[Indigo] \
    #text(purple)[Violet] \
  ],
)

#let color-box(name, bg-color) = {
  let text-color = if bg-color in (black, rgb("#001f3f"), rgb("#800033"), rgb("#3d9970")) { white } else { black }

  // Using layout() or fixed percentages to ensure it fits the 1in margins
  rect(
    width: 100%,
    height: 45pt, // Adjusted height to be slightly shorter for document flow
    fill: bg-color,
    stroke: if bg-color == white { 0.5pt + black } else { none },
    align(center + horizon, text(size: 8pt, fill: text-color, name)),
  )
}

#let navy = rgb("#001f3f")
#let silver = rgb("#dddddd")
#let eastern = rgb("#2491a5")
#let fuchsia = rgb("#ff00cc")
#let maroon = rgb("#800033")
#let olive = rgb("#3d9970")
#let lime = rgb("#00ff66")
#let aqua = rgb("#7fdbff")

#v(1em) // Spacing from header

#block(
  width: 100%,
  fill: white,
  inset: 15pt,
  stroke: 1pt + rgb("#eeeeee"),
  radius: 4pt,
  grid(
    columns: (1fr,) * 9,
    // Distributed evenly across the page width
    column-gutter: 6pt,
    row-gutter: 10pt,

    color-box("black", black),
    color-box("gray", gray),
    color-box("silver", silver),
    color-box("white", white),
    color-box("navy", navy),
    color-box("blue", blue),
    color-box("aqua", aqua),
    color-box("teal", teal),
    color-box("eastern", eastern),

    color-box("purple", purple),
    color-box("fuchsia", fuchsia),
    color-box("maroon", maroon),
    color-box("red", red),
    color-box("orange", orange),
    color-box("yellow", yellow),
    color-box("olive", olive),
    color-box("green", green),
    color-box("lime", lime),
  ),
)

A simple linear gradient from red to blue
#rect(width: 100%, height: 20pt, fill: gradient.linear(orange, blue))

A rainbow using a predefined color map
#rect(width: 100%, height: 20pt, fill: gradient.linear(..color.map.rainbow))

#text(fill: gradient.linear(..color.map.rainbow))[
  This entire sentence is a rainbow!]

#square(
  fill: oklch(40%, 0.2, 160deg, 50%),
)

#grid(
  columns: (1fr,) * 14,
  column-gutter: 4pt,

  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.turbo, angle: 90deg)),
    rotate(-90deg, reflow:true)[turbo],
  ),
   
    stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.cividis, angle: 90deg)),
    rotate(-90deg, reflow:true)[cividis],
  ),
  
  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.rainbow, angle: 90deg)),
    rotate(-90deg, reflow:true)[rainbow],
  ),
  
  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.spectral, angle: 90deg)),
    rotate(-90deg, reflow:true)[spectral],
  ),

  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.viridis, angle: 90deg)),
    rotate(-90deg, reflow:true)[viridis],
  ),

  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.inferno, angle: 90deg)),
    rotate(-90deg, reflow:true)[inferno],
  ),

  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.magma, angle: 90deg)),
    rotate(-90deg, reflow:true)[magma],
  ),

  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.plasma, angle: 90deg)),
    rotate(-90deg, reflow:true)[plasma],
  ),
  
  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.rocket, angle: 90deg)),
    rotate(-90deg, reflow:true)[rocket],
  ),

  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.mako, angle: 90deg)),
    rotate(-90deg, reflow:true)[cividis],
  ),

  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.vlag, angle: 90deg)),
    rotate(-90deg, reflow:true)[vlag],
  ),

  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.icefire, angle: 90deg)),
    rotate(-90deg, reflow:true)[icefire],
  ),

  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.flare, angle: 90deg)),
    rotate(-90deg, reflow:true)[flare],
  ),

  stack(
    spacing: 5pt,
    rect(width: 20pt, height: 70pt, fill: gradient.linear(..color.map.crest, angle: 90deg)),
    rotate(-90deg, reflow:true)[crest],
  ),

)


Sharp stripes instead of a smooth fade

#rect(
  width: 100%,
  height: 20pt,
  fill: gradient.linear(red, orange, yellow, lime, blue, rgb("#4B0082"), purple).sharp(7),
)

#text(fill: gradient.linear(
  (red, 0%),
  (yellow, 10%),
  (green, 20%),
  (lime, 80%),
  (blue, 100%),
))[Custom positions (0% to 100%)]

#rect(
  width: 100%,
  height: 20pt,
  fill: gradient.linear((red, 0%), (yellow, 10%), (green, 20%), (lime, 80%), (blue, 100%)),
)

#block(fill: red)[opaque]
#block(fill: red.transparentize(50%))[half red]
#block(fill: red.transparentize(75%))[quarter red]


#circle(radius: 25pt)

// With content.
#circle[
  #set align(center + horizon)
  Automatically \
  sized to fit.
]

#circle(fill: gradient.linear(orange, yellow))

#circle(
  radius: 40pt,
  fill: gradient.radial(aqua, white).repeat(4),
)

#curve(
  fill: blue.lighten(80%),
  stroke: blue,
  curve.move((0pt, 50pt)),
  curve.line((100pt, 50pt)),
  curve.cubic(none, (90pt, 0pt), (50pt, 0pt)),
  curve.close(),
)

#curve(
  fill: blue.lighten(80%),
  fill-rule: "even-odd",
  stroke: blue,
  curve.line((50pt, 0pt)),
  curve.line((50pt, 50pt)),
  curve.line((0pt, 50pt)),
  curve.close(),
  curve.move((10pt, 10pt)),
  curve.line((40pt, 10pt)),
  curve.line((40pt, 40pt)),
  curve.line((10pt, 40pt)),
  curve.close(),
)

Ellipse Without content.
#ellipse(width: 35%, height: 30pt, fill: red)

Ellipse With content.
#ellipse[
  #set align(center)
  Automatically sized \
  to fit the content.
]

#stack(
  dir: ltr,
  spacing: 1fr,
  square(fill: gradient.linear(..color.map.rainbow)),
  square(fill: gradient.radial(..color.map.rainbow)),
  square(fill: gradient.conic(..color.map.rainbow)),
)

#line(length: 100%)
#line(end: (50%, 50%))
#line(
  length: 4cm,
  stroke: 2pt + maroon,
)

polygon
#polygon(
  fill: blue.lighten(80%),
  stroke: blue,
  (20%, 0pt),
  (60%, 0pt),
  (80%, 2cm),
  (0%, 2cm),
)

// Without content.
#square(size: 40pt)

// With content.
#square[
  Automatically \
  sized to fit.
]

#set line(length: 100%)
#stack(
  spacing: 1em,
  line(stroke: 2pt + red),
  line(stroke: (paint: gradient.linear(orange, blue), thickness: 4pt, cap: "round")),
  line(stroke: (paint: blue, thickness: 1pt, dash: "dashed")),
  line(stroke: 2pt + gradient.linear(..color.map.rainbow)),
)

#let pat = tiling(size: (30pt, 30pt))[
  #place(line(start: (0%, 0%), end: (100%, 100%)))
  #place(line(start: (0%, 100%), end: (100%, 0%)))
]

#rect(fill: pat, width: 100%, height: 60pt, stroke: 1pt)
