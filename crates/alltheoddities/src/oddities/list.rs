//! `odd list`: each oddity, with its label and its hosts.

use crate::ctx::Ctx;
use crate::oddity::Oddity;

/// Prints a table, or JSON with the fields `name`, `label`, `hosts` and
/// `summary`.
pub fn run(oddities: &[&dyn Oddity], json: bool, ctx: &mut Ctx<'_>) {
    if json {
        let rows: Vec<serde_json::Value> = oddities
            .iter()
            .map(|o| {
                serde_json::json!({
                    "name": o.name(),
                    "label": o.label(),
                    "hosts": o.hosts(),
                    "summary": o.summary(),
                })
            })
            .collect();
        ctx.say(&serde_json::to_string_pretty(&rows).unwrap_or_default());
        return;
    }
    let width = oddities
        .iter()
        .map(|o| o.name().len())
        .max()
        .unwrap_or(0)
        .max(4);
    ctx.say(&format!(
        "{:width$}  {:12}  {:22}  WHAT IT DOES",
        "NAME", "LABEL", "HOSTS"
    ));
    for o in oddities {
        let hosts: Vec<String> = o.hosts().iter().map(ToString::to_string).collect();
        ctx.say(&format!(
            "{:width$}  {:12}  {:22}  {}",
            o.name(),
            o.label().word(),
            hosts.join(", "),
            o.summary()
        ));
    }
}
