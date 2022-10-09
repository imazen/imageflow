<!-- See https://github.com/check-spelling/check-spelling/wiki/Configuration-Examples%3A-advice --> <!-- markdownlint-disable MD033 MD041 -->
<details>
<summary>
:pencil2: Contributor please read this
</summary>

* You can probably just add items into `.github/actions/spell-check/expect/expect.txt`.
* If you need to use a specific token in one place and it shouldn't generally be used, you can
add an item in `.github/actions/spell-check/patterns.txt`.

<details><summary>If the flagged items are :exploding_head: false positives</summary>

If items relate to a ...
* binary file (or some other file you wouldn't want to check at all).

  Please add a file path to the `excludes.txt` file matching the containing file.

  File paths are Perl 5 Regular Expressions - you can [test](
https://www.regexplanet.com/advanced/perl/) yours before committing to verify it will match your files.

  `^` refers to the file's path from the root of the repository, so `^README\.md$` would exclude [README.md](
../tree/HEAD/README.md) (on whichever branch you're using).

* well-formed pattern.

  If you can write a [pattern](https://github.com/check-spelling/check-spelling/wiki/Configuration-Examples:-patterns) that would match it,
  try adding it to the `patterns.txt` file.

  Patterns are Perl 5 Regular Expressions - you can [test](
https://www.regexplanet.com/advanced/perl/) yours before committing to verify it will match your lines.

  Note that patterns can't match multiline strings.

</details>
</details>
