`picture-crop-mask.pptx` derives from the tracked demo deck. Only slide 1's picture and its bitmap change; slides 2 and 3 remain controls.

The 1024×1024 bitmap has RGB `(floor(x / 4), floor(y / 4), 40)`. The picture uses `srcRect l="10000" t="20000" r="30000" b="10000"`, an ellipse, and a 2px `#FF00FF` outline in the frame `(100, 50, 200, 100)` CSS pixels.

The kept source rectangle is `(102.4, 204.8, 614.4, 716.8)` pixels. Rendering should crop that rectangle, clip it to the ellipse, and stroke the ellipse. The adjacent before/after PNGs show the isolated picture on a 400×200 surface.
