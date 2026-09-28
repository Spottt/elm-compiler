//! Install diagnostics from Reporting.Exit in Elm 0.19.1 (LICENSE-ELM).
use crate::dependency_error::{reflow, report_encoded};
use serde_json::json;

pub(crate) fn unknown(package: &str, suggestions: &[String], online: bool) -> String {
    let first = reflow(&format!("I cannot find a package named {package}."));
    let explanation = if online {
        reflow(
            "I looked through https://package.elm-lang.org for packages with similar names and found these:",
        )
    } else {
        format!(
            "{}\n\nLooking through the locally cached names, the closest ones are:",
            reflow(
                "I could not connect to https://package.elm-lang.org though, so new packages may have been published since I last updated my local cache of package names."
            )
        )
    };
    let prefix = first.strip_suffix(&format!("{package}.")).unwrap();
    let report = json!({"type":"error","path":null,"title":"UNKNOWN PACKAGE","message":[prefix,
        {"bold":false,"underline":false,"color":"RED","string":package},
        format!(".\n\n{explanation}\n\n    "),
        {"bold":false,"underline":false,"color":"yellow","string":suggestions.join("\n    ")},
        "\n\nMaybe you want one of these instead?"]});
    format!("ELM_DEPENDENCY_JSON:{report}")
}

pub(crate) fn solution(error: String, package: &str, application: bool, online: bool) -> String {
    let Some(report) = report_encoded(&error) else {
        return error;
    };
    if !matches!(
        report["title"].as_str(),
        Some("INCOMPATIBLE DEPENDENCIES" | "TROUBLE VERIFYING DEPENDENCIES")
    ) {
        return error;
    }
    let first = reflow(&format!(
        "I cannot find a version of {package} that is compatible with your existing {}.",
        if application {
            "dependencies"
        } else {
            "constraints"
        }
    ));
    let title = if online {
        "CANNOT FIND COMPATIBLE VERSION"
    } else {
        "CANNOT FIND COMPATIBLE VERSION LOCALLY"
    };
    let paragraphs: &[&str] = if !online {
        &[
            "I was not able to connect to https://package.elm-lang.org/ though, so I was only able to look through packages that you have downloaded in the past.",
            "Try again later when you have internet!",
        ]
    } else if application {
        &[
            "I checked all the published versions. When that failed, I tried to find any compatible combination of these packages, even if it meant changing all your existing dependencies! That did not work either!",
            "This is most likely to happen when a package is not upgraded yet. Maybe a new version of Elm came out recently? Maybe a common package was changed recently? Maybe a better package came along, so there was no need to upgrade this one? Try asking around https://elm-lang.org/community to learn what might be going on with this package.",
            "Note: Whatever the case, please be kind to the relevant package authors! Having friendly interactions with users is great motivation, and conversely, getting berated by strangers on the internet sucks your soul dry. Furthermore, package authors are humans with families, friends, jobs, vacations, responsibilities, goals, etc. They face obstacles outside of their technical work you will never know about, so please assume the best and try to be patient and supportive!",
        ]
    } else {
        &[
            "With applications, I try to broaden the constraints to see if anything works, but messing with package constraints is much more delicate business. E.g. making your constraints stricter may make it harder for applications to find compatible dependencies. So fixing something here may break it for a lot of other people!",
            "So I recommend making an application with the same dependencies as your package. See if there is a solution at all. From there it may be easier to figure out how to proceed in a way that will disrupt your users as little as possible. And the solution may be to help other package authors to get their packages updated, or to drop a dependency entirely.",
        ]
    };
    let rest = paragraphs
        .iter()
        .map(|p| reflow(p))
        .collect::<Vec<_>>()
        .join("\n\n");
    let report = json!({"type":"error","path":"elm.json","title":title,"message":[format!("{first}\n\n{rest}")]});
    format!("ELM_DEPENDENCY_JSON:{report}")
}
