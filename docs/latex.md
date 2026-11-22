# LaTeX text

The canvas and SVG use display style at the text object's saved size. The algebra panel uses text style for LaTeX text objects, mathematical function definitions and numeric matrices. Its rows grow to fit the formula. Saved text keeps its original string, style and anchor through import and snapshot replay.

## Reference syntax

| Source | Syntax used in text |
| --- | --- |
| FormulaText and the LATEX StringTemplate | Scientific notation with `\cdot 10^{n}`, fractions, indexed radicals, powers, label subscripts, Greek letters, `\pi`, named operators, scaled pairs, sets, and parenthesized arrays. The serializer also uses `\overline`, `\sum`, `\prod`, `\int`, and the evaluated-at bar `\left.f(x)\right\vert_a^b`. |
| Vector and conditional printing | A vector can be a one-column `align` inside `\left( … \right)`. Decimal alignment uses an ampersand and negative `\hspace`. Multi-branch conditions use a left brace, a two-column array, and an invisible right delimiter. |
| TableText, FrequencyTable and StemPlot | `array` and `tabular`, aligned columns, fixed-width uppercase column specifications, row skips, `\hline`, column bars, foreground and cell backgrounds, and boxed legends. |
| Saved text and text commands | `<isLaTeX val="true"/>` selects the math renderer. Text includes handwritten markup, text and font commands, multiple lines, matrices, growing delimiters, accents, colors, `\rotatebox` and `\scalebox`. |

The syntax inventory comes from the EUPL-headed `AlgoLaTeX`, `StringTemplate`, `ExpressionSerializer`, `GeoList`, `LatexVectorPrinter`, `AlgoTableText`, `GeoText`, `DrawText`, and `DrawEquation` sources. The upstream serialization, algebra text and aural text tests supply representative inputs. [Source at 112919f](https://github.com/geogebra/geogebra/tree/112919f2d450ec7430a5cd3b65dba226fdc43b74/source/shared/common/src/main/java/org/geogebra/common)

## Layout

The renderer implements the math-list spacing, style progression, baseline boxes, fraction axis, scripts and limits described in Appendix G of *The TeXbook*. Its dimensions are expressed in ems because the bundled text fonts lack TeX font parameters. It stretches delimiters from the bundled glyphs and draws radical bars and accents as geometry. SVG writes glyph outlines and rules from the same layout used by egui. The font data and the outline reader are already in the application's dependency tree. [The TeXbook](https://cs.stanford.edu/people/knuth/abcde.html)

The parser handles `matrix`, `smallmatrix`, `pmatrix`, `bmatrix`, `Bmatrix`, `vmatrix`, `Vmatrix`, `array`, `tabular`, `cases`, `rcases`, `align`, `aligned`, `alignat`, `alignedat`, `gather`, `gathered`, `split`, and `eqnarray`, including their supported unnumbered forms. It supports standard size-changing styles, explicit math spacing, the reference palette, hex and RGB colors, and the `\fgcolor`, `\bgcolor` and `\cellcolor` commands.

The visual fixture is rendered with Metal at 1 point per pixel on Apple Silicon. The test permits 60 changed pixels above a 12-level channel delta. Layout, document round-trip, canvas, algebra, SVG, Java corpus and frame-counter tests run on the workspace's other targets.
