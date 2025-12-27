# Configuration

Losrs can be configured via a configuration file
(you can find the location of the file via `losrs config path`)
and via ENV variables.

See below for possible settings.

## output

### format

Format used when displaying cards via show and review commands.

[default: clean]

[possible values: clean, typst, logseq, sixel, kitty, i-term]

[ENV: LOSRS__OUTPUT__FORMAT]

### ppi

Pixel density in PPI (Pixels Per Inch).

Used with image based formats.

[default: 96]

[ENV: LOSRS__OUTPUT__PPI]

### base_font_size

"base" font size in points.

Everything is scaled relative to this font size.

It is best to set this to the same font size as your terminal font size.

Used with image based formats.

[default: 12]

[ENV: LOSRS__OUTPUT__BASE_FONT_SIZE]

### line_height_scaling

Height of the line relative to the font size.

Used for figuring out vertical size of the card.

Some terminals allow scaling line height (and some scale by default),
this value needs to be known to calculate the right image size.

It is best to set this to the same line height scaling
as your terminal line height scaling.

Used with image based formats.

[default: 1.2]

[ENV: LOSRS__OUTPUT__LINE_HEIGHT_SCALING]
