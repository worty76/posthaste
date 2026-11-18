# Bundled fonts

Three open-licence families ship with the app, subset to Latin, Latin
Extended and Vietnamese. Each file carries its SIL Open Font License 1.1
text as `OFL-<family>.txt`, committed beside it. `scripts/check-fonts.py`
must pass for every file here; the rule file (`ui/styles/AGENTS.md`)
requires it.

| Family | Files | Worn by |
|---|---|---|
| Be Vietnam Pro | `be-vietnam-pro-{400,500,600,700}.woff2` (static) | Blueberry |
| Nunito | `nunito-variable.woff2` (variable, wght 200–1000) | Sakura night |
| Quicksand | `quicksand-variable.woff2` (variable, wght 300–700) | Strawberry Milk |

The stylesheets use weights 400, 500, 600 and 650 (`grep -rhoE
"font-weight: *[0-9]+" ui/styles | sort -u`). The variable fonts cover 650
exactly; Be Vietnam Pro has no 650 static, so 650 resolves to Bold 700 by
CSS weight matching, which is why 700 ships.

## Provenance

Downloaded from [google/fonts](https://github.com/google/fonts), `main`
at commit `5bd87dfc525c8a0d1d8529d3684716e00c3fc59a`:

- `ofl/nunito/Nunito[wght].ttf`, `ofl/nunito/OFL.txt`
- `ofl/quicksand/Quicksand[wght].ttf`, `ofl/quicksand/OFL.txt`
- `ofl/bevietnampro/BeVietnamPro-{Regular,Medium,SemiBold,Bold}.ttf`,
  `ofl/bevietnampro/OFL.txt`

## Subsetting

Run by hand once per file; `fonttools` is a tool, not a dependency
(`pip3 install --user fonttools brotli`):

```sh
UNI="U+0000-00FF,U+0100-024F,U+0300-0301,U+0303-0304,U+0308-0309,U+0323,U+0329,U+1E00-1EFF,U+2000-206F,U+20AB,U+20AC,U+2122"
pyftsubset "Nunito[wght].ttf" --output-file=nunito-variable.woff2 --flavor=woff2 --unicodes="$UNI" --layout-features='*'
pyftsubset "Quicksand[wght].ttf" --output-file=quicksand-variable.woff2 --flavor=woff2 --unicodes="$UNI" --layout-features='*'
pyftsubset BeVietnamPro-Regular.ttf  --output-file=be-vietnam-pro-400.woff2 --flavor=woff2 --unicodes="$UNI" --layout-features='*'
pyftsubset BeVietnamPro-Medium.ttf   --output-file=be-vietnam-pro-500.woff2 --flavor=woff2 --unicodes="$UNI" --layout-features='*'
pyftsubset BeVietnamPro-SemiBold.ttf --output-file=be-vietnam-pro-600.woff2 --flavor=woff2 --unicodes="$UNI" --layout-features='*'
pyftsubset BeVietnamPro-Bold.ttf     --output-file=be-vietnam-pro-700.woff2 --flavor=woff2 --unicodes="$UNI" --layout-features='*'
```

Declared in `ui/styles/fonts.css`, imported from `ui/boot.js` before
`tokens.css`. The family names — `"Be Vietnam Pro"`, `"Nunito"`,
`"Quicksand"` — are what `tokens.css`'s stacks and the app's `FontFace`
variants use.
