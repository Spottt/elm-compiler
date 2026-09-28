//! Publication reports from Elm 0.19.1 Reporting.Exit (LICENSE-ELM).
use crate::{api_diff::Magnitude, dependency_error::reflow, package_publish::Problem};
use serde_json::{Value, json};
fn colored(color: &str, text: impl Into<String>) -> Value {
    json!({"bold":false,"underline":false,"color":color,"string":text.into()})
}
fn paragraph(text: &str, highlight: &str, color: &str) -> Vec<Value> {
    let text = reflow(text);
    match text.split_once(highlight) {
        Some((before, after)) => vec![json!(before), colored(color, highlight), json!(after)],
        None => vec![json!(text)],
    }
}
fn magnitude(value: Magnitude) -> &'static str {
    match value {
        Magnitude::Patch => "PATCH",
        Magnitude::Minor => "MINOR",
        Magnitude::Major => "MAJOR",
    }
}
fn report(title: &str, path: Option<&str>, message: Vec<Value>) -> String {
    format!(
        "ELM_DEPENDENCY_JSON:{}",
        json!({"type":"error","path":path,"title":title,"message":message})
    )
}
pub fn no_outline() -> String {
    report(
        "PUBLISH WHAT?",
        None,
        vec![json!(format!(
            "I cannot find an elm.json so I am not sure what you want me to publish.\n\n{}",
            reflow(
                "Elm packages always have an elm.json that states the version number, dependencies, exposed modules, etc."
            )
        ))],
    )
}
/// Internal I/O and documentation errors need their contextual reports at the caller.
pub fn problem(problem: &Problem) -> Option<String> {
    let (title, path, message) = match problem {
        Problem::Application => (
            "UNPUBLISHABLE",
            None,
            vec![json!("I cannot publish applications, only packages!")],
        ),
        Problem::NoExposedModules => {
            let mut m = paragraph(
                "To publish a package, the \"exposed-modules\" field of your elm.json must list at least one module.",
                "\"exposed-modules\"",
                "yellow",
            );
            m.push(json!(format!("\n\n{}",reflow("Which modules do you want users of the package to have access to? Add their names to the \"exposed-modules\" list."))));
            ("NO EXPOSED MODULES", Some("elm.json"), m)
        }
        Problem::NoSummary => {
            let mut m = paragraph(
                "To publish a package, your elm.json must have a \"summary\" field that gives a consice overview of your project.",
                "\"summary\"",
                "yellow",
            );
            m.push(json!(format!("\n\n{}",reflow("The summary must be less than 80 characters. It should describe the concrete use of your package as clearly and as plainly as possible."))));
            ("NO SUMMARY", Some("elm.json"), m)
        }
        Problem::NoReadme | Problem::ShortReadme => {
            let (title, summary) = if matches!(problem, Problem::NoReadme) {
                (
                    "NO README",
                    "Every published package must have a helpful README.md file, but I do not see one in your project.",
                )
            } else {
                (
                    "SHORT README",
                    "This README.md is too short. Having more details will help people assess your package quickly and fairly.",
                )
            };
            let m = vec![
                json!(format!(
                    "{}\n\nWhen people look at your README, they are wondering:\n\n  - What does this package even do?\n  - Will it help me solve MY problems?\n\n{}\n\n",
                    reflow(summary),
                    reflow(
                        "So I recommend starting your README with a small example of the most common usage scenario. Show people what they can expect if they learn more!"
                    )
                )),
                json!({"bold":false,"underline":true,"color":null,"string":"Note"}),
                json!(
                    &reflow(
                        "Note: By publishing your package, you are inviting people to invest time in understanding your work. Spending an hour on your README to communicate your knowledge more clearly can save the community days or weeks of time in aggregate, and saving time in aggregate is the whole point of publishing packages! People really appreciate it, and it makes the whole ecosystem feel nicer!"
                    )[4..]
                ),
            ];
            (title, Some("README.md"), m)
        }
        Problem::NoLicense => (
            "NO LICENSE FILE",
            Some("LICENSE"),
            vec![json!(format!(
                "{}\n\n{}",
                reflow(
                    "By publishing a package you are inviting the Elm community to build upon your work. But without knowing your license, we have no idea if that is legal!"
                ),
                reflow(
                    "Once you pick an OSI approved license from <https://spdx.org/licenses/>, you must share that choice in two places. First, the license identifier must appear in your elm.json file. Second, the full license text must appear in the root of your project in a file named LICENSE. Add that file and you will be all set!"
                )
            ))],
        ),
        Problem::NotInitialVersion(version) => {
            let mut m = paragraph(
                &format!("I cannot publish {version} as the initial version."),
                &version.to_string(),
                "RED",
            );
            m.push(json!("\n\n"));
            m.extend(paragraph(
                "Change it to 1.0.0 which is the initial version for all Elm packages.",
                "1.0.0",
                "GREEN",
            ));
            ("INVALID VERSION", None, m)
        }
        Problem::AlreadyPublished(version) => {
            let mut m = paragraph(
                &format!(
                    "Version {version} has already been published. You cannot publish it again!"
                ),
                &version.to_string(),
                "GREEN",
            );
            m.push(json!("\nTry using the `bump` command:\n\n"));
            m.push(colored("yellow", "    elm bump"));
            m.push(json!(format!("\n\n{}",reflow("It computes the version number based on API changes, ensuring that no breaking changes end up in PATCH releases!"))));
            ("ALREADY PUBLISHED", None, m)
        }
        Problem::InvalidBump { stated, latest } => {
            let mut m = paragraph(
                &format!(
                    "Your elm.json says the next version should be {stated}, but that is not valid based on the previously published versions."
                ),
                &stated.to_string(),
                "RED",
            );
            m.push(json!("\n\n"));
            m.extend(paragraph(&format!("Change the version back to {latest} which is the most recently published version. From there, have Elm bump the version by running:"),&latest.to_string(),"GREEN"));
            m.push(json!("\n\n    "));
            m.push(colored("GREEN", "elm bump"));
            m.push(json!(format!("\n\n{}",reflow("If you want more insight on the API changes Elm detects, you can run `elm diff` at this point as well."))));
            ("INVALID VERSION", Some("elm.json"), m)
        }
        Problem::BadBump {
            old,
            stated,
            stated_magnitude,
            required,
            actual_magnitude,
        } => {
            let mut m = paragraph(
                &format!(
                    "Your elm.json says the next version should be {stated}, indicating a {} change to the public API. This does not match the API diff given by:",
                    magnitude(*stated_magnitude)
                ),
                &stated.to_string(),
                "RED",
            );
            m.push(json!(format!("\n\n    elm diff {old}\n\n")));
            m.extend(paragraph(&format!("This command says this is a {} change, so the next version should be {required}. Double check everything to make sure you are publishing what you want!",magnitude(*actual_magnitude)),&required.to_string(),"GREEN"));
            m.push(json!(format!(
                "\n\n{}",
                reflow("Also, next time use `elm bump` and I'll figure all this out for you!")
            )));
            ("INVALID VERSION", Some("elm.json"), m)
        }
        Problem::Io(_) | Problem::InvalidDocumentation(_) => return None,
    };
    Some(report(title, path, message))
}

pub fn git(error: &crate::publish_git::Error) -> Option<String> {
    use crate::publish_git::Error;
    Some(match error {
        Error::MissingGit => report(
            "NO GIT",
            None,
            vec![
                json!(format!(
                    "{}\n\n{}\n\n",
                    reflow(
                        "I searched your PATH environment variable for `git` and could not find it. Is it available through your PATH?"
                    ),
                    reflow(
                        "Who cares about this? Well, I currently use `git` to check if there are any local changes in your code. Local changes are a good sign that some important improvements have gotten mistagged, so this check can be extremely helpful for package authors!"
                    )
                )),
                json!({"bold":false,"underline":true,"color":null,"string":"Note"}),
                json!(": We plan to do this without the `git` binary in a future release."),
            ],
        ),
        Error::MissingTag(version) => {
            let mut m = paragraph(
                &format!("Packages must be tagged in git, but I cannot find a {version} tag."),
                &version.to_string(),
                "GREEN",
            );
            m.push(json!("\n\nThese tags make it possible to find this specific version on GitHub.\nTo tag the most recent commit and push it to GitHub, run this:\n\n    "));
            m.push(colored(
                "yellow",
                format!("git tag -a {version} -m \"new release\"\n    git push origin {version}"),
            ));
            m.push(json!(
                "\n\nThe -m flag is for a helpful message. Try to make it more informative!"
            ));
            report("NO TAG", None, m)
        }
        Error::LocalChanges(version) => {
            let mut m = paragraph(
                &format!(
                    "The code tagged as {version} in git does not match the code in your working directory. This means you have commits or local changes that are not going to be published!"
                ),
                &version.to_string(),
                "GREEN",
            );
            m.push(json!("\n\n"));
            m.push(json!({"bold":false,"underline":true,"color":null,"string":"Note"}));
            m.push(json!(&reflow(&format!("Note: If you are sure everything is in order, you can run `git checkout {version}` and publish your code from there."))[4..]));
            report("LOCAL CHANGES", None, m)
        }
        Error::Io(_) => return None,
    })
}

pub fn bad_archive_build() -> String {
    report(
        "PROBLEM VERIFYING PACKAGE",
        None,
        vec![json!(format!(
            "{}\n\n{}",
            reflow(
                "Before publishing packages, I download the code from GitHub and try to build it from scratch. That way I can be more confident that it will work for other people too. But I am not able to build it!"
            ),
            reflow(
                "I was just able to build your local copy though. Is there some way the version on GitHub could be different?"
            )
        ))],
    )
}
/// Context is one of the publication phases; transport exception wording is
/// retained from Rust and is not claimed to match Haskell's HTTP library.
pub fn network(
    error: crate::publish_network::Error,
    version: crate::package_solver::Version,
    phase: &str,
) -> String {
    use crate::publish_network::Error;
    match error {
        Error::TagData { url, body } => crate::diff_diagnostic::invalid_response(
            "PROBLEM VERIFYING TAG",
            &format!("I need to check that version {version} is tagged on GitHub"),
            &url,
            &body,
        ),
        Error::Http {
            status: Some(404), ..
        } if phase == "tag" => {
            let mut m = vec![json!(format!(
                "You have version {version} tagged locally, but not on GitHub.\n\nRun the following command to make this tag available on GitHub:\n\n    "
            ))];
            m.push(colored("yellow", format!("git push origin {version}")));
            m.push(json!(format!("\n\n{}",reflow("This will make it possible to find your code online based on the version number."))));
            report("NO TAG ON GITHUB", None, m)
        }
        Error::Archive { url, .. } => report(
            "PROBLEM DOWNLOADING CODE",
            None,
            vec![
                json!(format!(
                    "{}\n\n    ",
                    reflow(
                        "I need to check that folks can download and build the source code when they install this package, so I downloaded the code from:"
                    )
                )),
                colored("yellow", url),
                json!(format!(
                    "\n\n{}",
                    reflow(
                        "I was unable to unzip the archive though. Maybe there is something weird with your internet connection. We have gotten reports that schools, businesses, airports, etc. sometimes intercept requests and add things to the body or change its contents entirely. Could that be the problem?"
                    )
                )),
            ],
        ),
        Error::Http { url, detail, .. } => {
            let (title, context) = match phase {
                "tag" => (
                    "PROBLEM VERIFYING TAG",
                    "I need to check that the version tag is registered on GitHub",
                ),
                "archive" => (
                    "PROBLEM DOWNLOADING CODE",
                    "I need to check that folks can download and build the source code when they install this package",
                ),
                _ => (
                    "PROBLEM PUBLISHING PACKAGE",
                    "I need to send information about your package to the package website",
                ),
            };
            let encoded = crate::diff_diagnostic::registry_context(context, &url, &detail);
            let mut value =
                crate::dependency_error::report_encoded(&encoded).expect("registry report");
            value["title"] = json!(title);
            format!("ELM_DEPENDENCY_JSON:{value}")
        }
        Error::Io(detail) => detail,
    }
}
