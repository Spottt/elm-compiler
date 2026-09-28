//! Standalone HTML shell for a single compiled Elm entry.
pub fn render(module: &str, javascript: &str) -> String {
    // Generated JavaScript can contain Elm strings with HTML closing tags.
    // Prevent the HTML parser from ending the script while preserving JS values.
    let javascript = javascript.replace("</", "<\\/");
    let access = module
        .split('.')
        .map(|part| format!("[{}]", serde_json::to_string(part).unwrap()))
        .collect::<String>();
    format!(
        r#"<!DOCTYPE HTML>
<html>
<head>
  <meta charset="UTF-8">
  <title>{module}</title>
  <style>body {{ padding: 0; margin: 0; }}</style>
</head>
<body>
<pre id="elm"></pre>
<script>
try {{
{javascript}
  var app = Elm{access}.init({{ node: document.getElementById("elm") }});
}} catch (e) {{
  var header = document.createElement("h1");
  header.style.fontFamily = "monospace";
  header.innerText = "Initialization Error";
  var pre = document.getElementById("elm");
  document.body.insertBefore(header, pre);
  pre.innerText = e;
  throw e;
}}
</script>
</body>
</html>
"#
    )
}
