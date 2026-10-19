# Localization / Localização

EarthCraft embeds [English](../crates/earthcraft-core/locales/en.ftl) and
[Brazilian Portuguese](../crates/earthcraft-core/locales/pt-BR.ftl) Fluent catalogs.
Editing them requires rebuilding; no translation files need to be installed next
to the executable. **Settings → Language** defaults to **System Locale** and
resolves the OS locale at startup and when the selection changes. Portuguese
locales use Brazilian Portuguese; unsupported or unknown locales use English.
The System option includes the resolved language, for example **System locale
(English)** or **System locale (Português (Brasil))**, even when an explicit
override is active. Unsupported or unknown locales show **System locale
(English — fallback)**. These whole labels are localized in both catalogs.
The existing `lang.txt` file stores `system`, `en` or `pt-BR`. Existing `en` and
`pt-BR` values remain explicit overrides, while missing/unknown preferences select
System Locale. Persist the preference, never the system choice's resolved language,
so a later OS language change is respected on the next launch. Resolution belongs
to core; OS detection stays in the desktop and file I/O stays in data.

Name keys after the feature and message role: `settings-title`,
`search-input-placeholder`, `wayback-fallback-help`. A key stays the same when
wording changes. Keep names concise; avoid sentence slugs, duplicated feature
names and truncated prose. Share a key when its meaning and usage are the same,
rather than merely because two labels currently have identical text.

Add the same semantic ID and named variables to both files:

```ftl
# en.ftl
search-navigation-status = Going to { $destination }

# pt-BR.ftl
search-navigation-status = Indo para { $destination }
```

Rust calls the language API in core:

```rust
let label = lang.text("layers-section-title");
let status = lang.format("search-navigation-status", &[("destination", name.as_str().into())]);
```

Use `text` for parameterless labels and `format` for whole messages. Keep
punctuation and word order in Fluent rather than joining translated fragments.
Static labels are cached after first use; the shared concurrent bundles can also
be used by background KML imports. There is no runtime file I/O.

Pass preformatted coordinates as strings to preserve decimal precision. Use
numeric counts and Fluent select expressions for singular/plural messages rather
than parenthesized endings such as `result(s)`. Search and KML import messages
cover zero, one and multiple items; tests retain ungrouped counts. Fluent isolation is disabled for the current English/Portuguese
catalogs to retain output. Review bidirectional text handling before
introducing an RTL language.

Missing or unresolvable translations fall back to English. If neither locale can
resolve the message, the ID is displayed. Embedded syntax/duplicate-key mistakes
fail initialization; automated tests catch these before release. Catalog parity
is required even though runtime fallback exists. Native language names and
provider-supplied geographic names remain metadata. Application-owned diagnostic
logs and error messages use English. Error statuses explicitly select `Lang::En`,
regardless of the UI language; normal labels and success messages use the selected
locale. External source-error details are passed through unchanged.

Run `cargo test -p earthcraft-core --locked`. Tests verify syntax, matching IDs
and variables, resolution, fallback, application/data references and exact
original labels and dynamic-message output. Regression fixtures are explained in
[their README](../crates/earthcraft-core/tests/fixtures/README.md). Do not update
expected output just to make a regression check pass. Also switch languages
in the desktop app and inspect settings, help, search, file status and provider
attribution when those areas change.

## Português

Nomeie as chaves pelo recurso e pela função da mensagem, como `settings-title`
ou `search-input-placeholder`. Mantenha nomes curtos e estáveis quando o texto
mudar; não transforme frases traduzidas em chaves nem repita o nome do recurso.
Compartilhe uma chave apenas quando o significado e o uso forem os mesmos.
Adicione as mesmas chaves e variáveis nos dois arquivos FTL. Use `Lang::text`
para rótulos estáticos e `Lang::format` para mensagens completas com argumentos.
Pontuação e ordem das palavras pertencem ao catálogo. Os arquivos ficam embutidos
no executável e exigem uma recompilação depois de editar.

Mantenha coordenadas pré-formatadas como strings para conservar a precisão.
O seletor em **Configurações → Idioma** usa o idioma do sistema por padrão,
com inglês como fallback. O arquivo `lang.txt` mantém `system`, `en` ou `pt-BR`;
preferências antigas explícitas são preservadas. A opção `system` é resolvida
novamente ao iniciar, sem ser substituída pelo idioma detectado.
Use contagens numéricas e expressões de seleção do Fluent para singular e plural,
sem terminações entre parênteses como `resultado(s)`. Mensagens ausentes usam inglês como fallback; falhas nas duas
línguas mostram a chave. Os testes verificam sintaxe, chaves, variáveis, resolução
e os textos originais. Execute os testes de core e confira a troca de idioma na
interface. Nomes geográficos continuam vindos dos serviços. Logs e mensagens de erro do
aplicativo usam inglês, inclusive quando a interface está em português; detalhes
de erros externos são preservados.

Reference checks parse Rust syntax in app, data and core sources. They cover
rustfmt-wrapped calls, literal message IDs inside match expressions, static
`*_IDS` tables and literal named-argument arrays passed to `Lang::format`.
Production references are checked against the catalog schema; deliberately
invalid test fixtures are excluded. Dynamic IDs/argument arrays still need
explicit domain fixtures. `syn` is a test-only dependency; runtime localization
is unchanged.
