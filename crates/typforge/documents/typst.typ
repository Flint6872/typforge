#set page(width: 210mm, height: 297mm, margin: 2cm)
#set text(font: "Courier New", lang: "en")
#let rainbow(content) = {
  set text(fill: gradient.linear(..color.map.rainbow))
  box(content)
}


#figure(
  image("img/chef-man.svg", width: 20%),
  caption: [
    A step in the molecular testing
    pipeline of our lab.
  ],
)

//#include "hello.typ"

$ 5/3+2 = "Fish" $
$ pi * pi = 6. $
$ A = pi r^2 $
$ "area" = pi dot "radius"^2 $
$
  cal(A) :=
  { x in RR | x "is natural" }
$
#let x = 5
$ #x < 17 $

#rect(
  width: 100%,
  height: 20pt,
  fill: gradient.linear(
    ..color.map.viridis,
  ),
)


$
  sum_(k=0)^n k & = 1 + ... + n \
                & = (n(n+1)) / 2
$

= Hello, Typst!

This is your first *GPUI-Typst* document.
It combines the power of #link("https://typst.app")[Typst] for typesetting
with the low-latency UI of #link("https://github.com/zed-industries/gpui")[GPUI].

This is a gradient on text, but with a #rainbow[twist]!

#grid(
  columns: 2,
  rows: 2,
  column-gutter: 1em,
  row-gutter: 1em,

  [
    *Phase 1: Skeleton*
    - GPUI Window
    - Typst Rendering
    - Display Image
  ],
  [
    *Phase 2: Interaction*
    - Coordinate Mapping
    - Cursors
    - Hit-Testing
  ],

  [
    *Phase 3: Collaboration*
    - CRDT
    - Networking
    - Remote Cursors
  ],
  [
    *Future Features*
    - Virtual Scrolling
    - Incremental Rendering
  ],
)


$ A = pi r^2 $
$ "area" = pi dot "radius"^2 $
$
  cal(A) :=
  { x in RR | x "is natural" }
$
#let x = 5

$ #x < 17 $

#rect(
  width: 100%,
  height: 20pt,
  fill: gradient.linear(
    ..color.map.viridis,
  ),
)


$
  sum_(k=0)^n k & = 1 + ... + n \
                & = (n(n+1)) / 2
$

= Hello, Typst!

This is your first *GPUI-Typst* document.
It combines the power of #link("https://typst.app")[Typst] for typesetting
with the low-latency UI of #link("https://github.com/zed-industries/gpui")[GPUI].

This is a gradient on text, but with a #rainbow[twist]!
