#import "@preview/keepsake:0.1.0": primary-content, quarter-fold-vertical

#let paper = "us-letter"

#let front = [
  #align(center + horizon)[
    #text(size: 32pt, weight: "bold")[Happy Birthday!]
    #image("img/chef-man.svg", width: 20%),
  ]
]

#let primary = primary-content(
  greeting: "Dear Friend,",
  body: [Wishing you a wonderful year ahead.],
  sender: "Your Name",
)

#let back = [
  #align(center + bottom)[Made with Typst]
]

#show: quarter-fold-vertical.with(
  paper: paper,
  front: front,
  primary: primary,
  back: back,
)
