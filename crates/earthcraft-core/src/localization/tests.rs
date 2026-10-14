use super::*;
use fluent_syntax::{ast, parser};
use std::collections::{BTreeMap, BTreeSet};

fn variables(pattern: &ast::Pattern<&str>, out: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            expression_variables(expression, out);
        }
    }
}
fn expression_variables(expression: &ast::Expression<&str>, out: &mut BTreeSet<String>) {
    match expression {
        ast::Expression::Inline(inline) => inline_variables(inline, out),
        ast::Expression::Select { selector, variants } => {
            inline_variables(selector, out);
            for variant in variants {
                variables(&variant.value, out);
            }
        }
    }
}
fn inline_variables(inline: &ast::InlineExpression<&str>, out: &mut BTreeSet<String>) {
    match inline {
        ast::InlineExpression::VariableReference { id } => {
            out.insert(id.name.to_owned());
        }
        ast::InlineExpression::Placeable { expression } => expression_variables(expression, out),
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            argument_variables(arguments, out)
        }
        ast::InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => argument_variables(arguments, out),
        _ => {}
    }
}
fn argument_variables(args: &ast::CallArguments<&str>, out: &mut BTreeSet<String>) {
    for value in &args.positional {
        inline_variables(value, out);
    }
    for named in &args.named {
        inline_variables(&named.value, out);
    }
}
fn schema(source: &str) -> BTreeMap<String, BTreeSet<String>> {
    let resource = parser::parse(source).expect("valid catalog syntax");
    let mut schema = BTreeMap::new();
    for entry in resource.body {
        if let ast::Entry::Message(message) = entry {
            let mut args = BTreeSet::new();
            variables(&message.value.expect("message value"), &mut args);
            for attribute in &message.attributes {
                variables(&attribute.value, &mut args);
            }
            assert!(
                schema.insert(message.id.name.to_owned(), args).is_none(),
                "duplicate id"
            );
        }
    }
    schema
}
#[test]
fn catalogs_have_matching_keys_variables_and_format_every_message() {
    let en = schema(EN);
    assert_eq!(en, schema(PT));
    for (id, variables) in en {
        let args: FluentArgs = variables
            .iter()
            .map(|name| (name.as_str(), "fixture"))
            .collect();
        for catalog in catalogs() {
            assert!(
                catalog.format(&id, Some(&args)).is_some(),
                "unresolved message {id}"
            );
        }
    }
}
#[test]
fn all_original_labels_keep_exact_wording_and_help_newlines() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/original-ui-strings.json"
    ))
    .unwrap();
    for (id, values) in fixture.as_object().unwrap() {
        for lang in Lang::ALL {
            // Upstream added worldwide OSM municipal borders after the original fixture.
            let expected = match (id.as_str(), lang) {
                ("layers-municipal-borders-toggle", Lang::En) => {
                    "Municipal borders worldwide (OSM), Brazil (IBGE)"
                }
                ("layers-municipal-borders-toggle", Lang::PtBr) => {
                    "Limites de municípios do mundo (OSM) e Brasil (IBGE)"
                }
                _ => values[lang.code()].as_str().unwrap(),
            };
            assert_eq!(lang.text(id), expected, "{id}");
        }
    }
}
#[test]
fn dynamic_messages_preserve_counts_paths_spacing_and_precision() {
    let args = [
        ("count", 1234usize.into()),
        ("path", "a & b/locais.kml".into()),
    ];
    assert_eq!(
        Lang::En.format("kml-import-success", &args),
        "1234 placemarks imported. — a & b/locais.kml"
    );
    assert_eq!(
        Lang::PtBr.format("kml-import-success", &args),
        "1234 locais importados. — a & b/locais.kml"
    );
    assert_eq!(
        Lang::PtBr.format(
            "search-navigation-status",
            &[("destination", "-23.55000, -46.63000".into())]
        ),
        "Indo para -23.55000, -46.63000"
    );
    assert_eq!(
        Lang::En.format(
            "ruler-measurement-summary",
            &[("count", "2".into()), ("distance", "1.50 km".into())]
        ),
        "Points: 2   Total: 1.50 km"
    );
    assert_eq!(
        Lang::PtBr.format("search-request-error", &[("error", "HTTP 500".into())]),
        "Erro na busca: HTTP 500"
    );
}
#[test]
fn counted_messages_use_singular_only_for_one_in_both_languages() {
    for (count, en_search, pt_search, en_import, pt_import) in [
        (
            0usize,
            "0 results found.",
            "0 resultados encontrados.",
            "0 placemarks imported. — points.kml",
            "0 locais importados. — points.kml",
        ),
        (
            1,
            "1 result found.",
            "1 resultado encontrado.",
            "1 placemark imported. — points.kml",
            "1 local importado. — points.kml",
        ),
        (
            2,
            "2 results found.",
            "2 resultados encontrados.",
            "2 placemarks imported. — points.kml",
            "2 locais importados. — points.kml",
        ),
    ] {
        let args = [("count", count.into()), ("path", "points.kml".into())];
        assert_eq!(Lang::En.format("search-results-status", &args), en_search);
        assert_eq!(Lang::PtBr.format("search-results-status", &args), pt_search);
        assert_eq!(Lang::En.format("kml-import-success", &args), en_import);
        assert_eq!(Lang::PtBr.format("kml-import-success", &args), pt_import);
    }
}
#[test]
fn missing_or_unformattable_translations_fall_back() {
    let en = Catalog::new("en", "hello = Hello\nwelcome = Hello { $name }\n");
    let pt = Catalog::new("pt-BR", "welcome = Olá { $missing }\n");
    let args = FluentArgs::from_iter([("name", "EarthCraft")]);
    assert_eq!(
        pt.format_or_fallback(&en, "hello", None).as_deref(),
        Some("Hello")
    );
    assert_eq!(
        pt.format_or_fallback(&en, "welcome", Some(&args))
            .as_deref(),
        Some("Hello EarthCraft")
    );
    assert_eq!(Lang::PtBr.text("unknown-key"), "unknown-key");
    assert_eq!(
        Lang::PtBr.format("search-navigation-status", &[]),
        "search-navigation-status"
    );
}

#[test]
fn original_dynamic_messages_preserve_output_except_requested_pluralization() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/original-ui-messages.json"
    ))
    .unwrap();
    for (id, values) in fixture.as_object().unwrap() {
        let args: Vec<(&str, FluentValue)> = values["args"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| {
                let value = value.as_str().unwrap();
                if name == "count"
                    && matches!(id.as_str(), "search-results-status" | "kml-import-success")
                {
                    (name.as_str(), value.parse::<usize>().unwrap().into())
                } else {
                    (name.as_str(), value.into())
                }
            })
            .collect();
        for lang in Lang::ALL {
            // These two messages intentionally replace parenthesized plurals.
            // Keep the original fixture intact as provenance for all other output.
            let expected = match (id.as_str(), lang) {
                ("search-results-status", Lang::En) => "1234 results found.",
                ("search-results-status", Lang::PtBr) => "1234 resultados encontrados.",
                ("kml-import-success", Lang::En) => "1234 placemarks imported. — a & b/locais.kml",
                ("kml-import-success", Lang::PtBr) => "1234 locais importados. — a & b/locais.kml",
                _ => values[lang.code()].as_str().unwrap(),
            };
            assert_eq!(lang.format(id, &args), expected, "{id}");
        }
    }
}

#[test]
fn application_and_data_message_references_exist() {
    fn check(path: &std::path::Path, keys: &BTreeMap<String, BTreeSet<String>>) {
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                check(&entry.unwrap().path(), keys);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let source = std::fs::read_to_string(path).unwrap();
            for method in [".text(\"", ".format(\""] {
                for rest in source.split(method).skip(1) {
                    let id = rest.split('"').next().unwrap();
                    assert!(keys.contains_key(id), "missing {id} in {}", path.display());
                }
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let keys = schema(EN);
    check(&root.join("apps/earthcraft/src"), &keys);
    check(&root.join("crates/earthcraft-data/src"), &keys);
}

#[test]
fn explorer_labels_follow_selected_language_and_keep_variable_interpolation() {
    assert_eq!(
        Lang::En.text("explorer-selected-location"),
        "Selected location"
    );
    assert_eq!(
        Lang::PtBr.text("explorer-selected-location"),
        "Local selecionado"
    );
    assert_eq!(Lang::En.text("top-explore-action"), "Explore +");
    assert_eq!(Lang::PtBr.text("top-explore-action"), "Explorar +");
    assert_eq!(Lang::En.text("explorer-bodies"), "Celestial bodies");
    assert_eq!(Lang::PtBr.text("explorer-bodies"), "Corpos celestes");
    assert_eq!(
        Lang::En.text("explorer-cloud-credit"),
        "Cloud imagery: Matt Eason / Live Cloud Maps"
    );
    assert_eq!(
        Lang::PtBr.text("explorer-cloud-credit"),
        "Imagens de nuvens: Matt Eason / Live Cloud Maps"
    );
    assert_eq!(
        Lang::En.format(
            "explorer-tour-default",
            &[("place", "Rio de Janeiro".into())]
        ),
        "We are exploring Rio de Janeiro. Explore the location on the globe and choose points of interest to continue your tour."
    );
    assert_eq!(
        Lang::PtBr.format(
            "explorer-tour-default",
            &[("place", "Rio de Janeiro".into())]
        ),
        "Estamos explorando Rio de Janeiro. Explore o local no globo e escolha pontos de interesse para continuar seu passeio."
    );
}
