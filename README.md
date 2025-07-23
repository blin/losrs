# logseq-srs

*logseq-srs* lets you review Spaced Repetition System (SRS) cards created in
[Logseq](https://github.com/logseq/logseq) in the terminal.

This project was created as a workaround for
[Logseq SRS Algorithm being faulty](https://github.com/logseq/logseq/issues/8890)
and the fault
[only being fixed in the database version of Logseq](https://github.com/logseq/logseq/pull/11540)
to the exclusion of plain file version of Logseq.

TODO: asciicinema a review session.

## Prerequisites

logseq-srs works with a very narrow subset of Logseq features,
at the very least you need to ensure that:
Logseq graph root where you plan to use logseq-srs is configured with
`:export/bullet-indentation :two-spaces` .

To render images in your cards you can use logseq-srs with `--format=sixel`.

That format requires:

1. Your terminal to support the sixel format.
   See [Are We Sixel Yet?](https://www.arewesixelyet.com/).
2. [Pandoc](https://github.com/jgm/pandoc) available on the `$PATH`
3. [Typst](https://github.com/typst/typst) available on the `$PATH`
4. `img2sixel`
   (via [libsixel](https://github.com/saitoha/libsixel))
   available on the `$PATH`.
   There is an outstanding `TODO` to not require `img2sixel`.

Rendering pipeline is basically:

```text
markdown -> typst -> png -> sixel
```

And was borrowed from [presenterm](https://github.com/mfontanini/presenterm).
The
[reasons presenterm has for converting LaTeX to Typst](https://github.com/mfontanini/presenterm/blob/master/docs/src/features/code/latex.md?plain=1#L30)
do not apply to this project, but I found it easy to work with this pipeline
so I'm keeping it. It would be nice to remove the dependency on Typst
and to support more LaTeX, but I am unlikely to get around to changing this
any time soon.

## Limitations

Things that are known to NOT work:

* Rendering references
* Rendering LaTeX code that can not be converted to Typst
