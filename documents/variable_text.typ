#set page(paper: "a4", margin: 2cm)
#set text(font: "PT Sans", size: 11pt)

= Typst Font Management & Fallbacks

When processing text, Typst tries all specified font families in order until it finds a font that has the necessary glyphs. 

== 1. Basic Fallbacks
If a font doesn't contain the requested character (such as Arabic glyphs missing from a Latin-based font), Typst moves down the priority list.

#set text(font: ("Inria Serif", "Noto Sans Arabic"))
This is Latin text using Inria Serif. 

هذا عربي (This text falls back to Noto Sans Arabic).

#v(1em)

== 2. Character-Specific Overrides
You can use a dictionary inside the font array to apply a specific font only to certain characters, such as numbers.

#set text(font: (
  (name: "PT Sans", covers: regex("[0-9]")),
  "Libertinus Serif"
))
The number 123 uses PT Sans, while this surrounding text uses Libertinus Serif.

#v(1em)

== 3. Mixing Latin and CJK Fonts
Typst’s `covers` parameter can restrict a font to specific writing systems (like CJK), while allowing another font to handle the rest.

#set text(font: (
  (name: "Inria Serif", covers: "latin-in-cjk"),
  "Noto Serif CJK SC"
))
In this sentence we分別設置了“中文”和 English typography simultaneously.

#v(1em)

== 4. Variable Fonts & Naming
Typst unifies different fonts from the same family. It automatically trims suffixes like *“Bold”*, *“Condensed”*, *“Variable”*, *“Var”*, and *“VF”* from font family names. 

To apply these variations, access them using Typst's built-in parameters such as `weight`, `stretch`, and `style` instead of using the full font name. Typst will automatically pick the closest matching static or variable font, preferring variable fonts when both are available.

*Note:* If you are using the Typst CLI, Typst detects your system fonts or its embedded ones (Liberinus Serif, DejaVu Sans Mono, New Computer Modern). You can use `--font-path` or `TYPST_FONT_PATHS` to add more directories. In the Web App, uploading `.ttf` or `.otf` files automatically makes them available.
