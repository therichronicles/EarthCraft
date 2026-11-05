//! Parse Rust expressions so rustfmt wrapping cannot hide localization mistakes.
use super::{BTreeMap, BTreeSet, EN, schema};
use syn::{Expr, Lit, visit::Visit};

type Catalog = BTreeMap<String, BTreeSet<String>>;
fn literal(expression: &Expr) -> Option<String> {
    if let Expr::Lit(value) = expression
        && let Lit::Str(value) = &value.lit
    {
        Some(value.value())
    } else {
        None
    }
}
fn message_ids(expression: &Expr) -> Vec<String> {
    match expression {
        Expr::Lit(_) => literal(expression).into_iter().collect(),
        Expr::Match(value) => value
            .arms
            .iter()
            .flat_map(|arm| message_ids(&arm.body))
            .collect(),
        Expr::Array(value) => value.elems.iter().flat_map(message_ids).collect(),
        Expr::Reference(value) => message_ids(&value.expr),
        Expr::Paren(value) => message_ids(&value.expr),
        _ => Vec::new(),
    }
}
fn argument_names(expression: &Expr) -> Option<BTreeSet<String>> {
    match expression {
        Expr::Reference(value) => argument_names(&value.expr),
        Expr::Paren(value) => argument_names(&value.expr),
        Expr::Array(value) => value
            .elems
            .iter()
            .map(|element| {
                let Expr::Tuple(tuple) = element else {
                    return None;
                };
                literal(tuple.elems.first()?)
            })
            .collect(),
        _ => None,
    }
}
struct References<'a> {
    catalog: &'a Catalog,
    errors: Vec<String>,
}
impl References<'_> {
    fn check_id(&mut self, id: &str) {
        if !self.catalog.contains_key(id) {
            self.errors.push(format!("missing message {id}"));
        }
    }
}
impl<'ast> Visit<'ast> for References<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if item.attrs.iter().any(|attribute| {
            attribute
                .path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "test")
        }) {
            return;
        }
        syn::visit::visit_item_fn(self, item);
    }
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if item.attrs.iter().any(|attribute| attribute.path().is_ident("cfg")
            && matches!(&attribute.meta, syn::Meta::List(list) if list.tokens.to_string().split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == "test"))) {
            return;
        }
        syn::visit::visit_item_mod(self, item);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let method = call.method.to_string();
        if (method == "text" || method == "format")
            && let Some(expression) = call.args.first()
        {
            for id in message_ids(expression) {
                self.check_id(&id);
                if method == "format"
                    && let Some(expected) = self.catalog.get(&id)
                    && let Some(actual) = call.args.iter().nth(1).and_then(argument_names)
                    && &actual != expected
                {
                    self.errors.push(format!(
                        "{id} arguments: expected {expected:?}, got {actual:?}"
                    ));
                }
            }
        }
        syn::visit::visit_expr_method_call(self, call);
    }
    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if item.ident.to_string().ends_with("_IDS") {
            for id in message_ids(&item.expr) {
                self.check_id(&id);
            }
        }
        syn::visit::visit_item_const(self, item);
    }
}
fn reference_errors(source: &str, catalog: &Catalog) -> Vec<String> {
    let file = syn::parse_file(source).expect("valid Rust source");
    let mut references = References {
        catalog,
        errors: Vec::new(),
    };
    references.visit_file(&file);
    references.errors
}
#[test]
fn wrapped_calls_arguments_and_id_tables_are_checked() {
    let catalog = BTreeMap::from([
        ("message".into(), BTreeSet::from(["name".into()])),
        ("static-message".into(), BTreeSet::new()),
    ]);
    assert!(
        reference_errors(
            r#"fn f() { lang.format(
        "message", &[("name", value.into())]); }"#,
            &catalog
        )
        .is_empty()
    );
    for source in [
        r#"fn f() { lang.format(
        "missing", &[]); }"#,
        r#"fn f() { lang.format("message", &[("typo", value.into())]); }"#,
        r#"const DIRECTION_IDS: [&str; 1] = ["missing"];"#,
        r#"fn f() { lang.text(match value { A => "static-message", B => "missing" }); }"#,
    ] {
        assert!(!reference_errors(source, &catalog).is_empty(), "{source}");
    }
}
#[test]
fn application_data_and_core_references_exist_with_matching_literal_arguments() {
    fn check(path: &std::path::Path, catalog: &Catalog) {
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                check(&entry.unwrap().path(), catalog);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let errors = reference_errors(&std::fs::read_to_string(path).unwrap(), catalog);
            assert!(
                errors.is_empty(),
                "{}: {}",
                path.display(),
                errors.join("; ")
            );
        }
    }
    let catalog = schema(EN);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for source in [
        "apps/earthcraft/src",
        "crates/earthcraft-data/src",
        "crates/earthcraft-core/src",
    ] {
        check(&root.join(source), &catalog);
    }
    for model in crate::earth_model::EarthModel::ALL {
        for id in [model.name_id(), model.help_id()] {
            assert!(catalog.contains_key(id), "{id}");
        }
    }
    for unit in crate::units::RulerUnit::ALL {
        for lang in crate::language::Lang::ALL {
            let label = unit.label(lang);
            assert!(!label.is_empty());
            assert!(!label.starts_with("ruler-unit-"));
        }
    }
}
