sintaxis llaves
pub struct TransactionalEmail<'content> {
    pub display_name: &'content str,
    pub preheader: &'content str,
    pub title: &'content str,
    pub message: &'content str,
    pub verification_code: Option<&'content str>,
    pub action: Option<(&'content str, &'content str)>,
    pub footer_note: Option<&'content str>,
}

impl TransactionalEmail<'_> {
    pub fn render(&self) -> (String, String) {
        let verification_code_html = self
            .verification_code
            .map(|code| {
                format!(
                    "<div style=\"margin:28px 0;padding:20px 24px;background:#f5f3ff;border:1px solid #e9e5ff;border-radius:12px;text-align:center\"><p style=\"margin:0 0 8px;color:#625f75;font-size:13px\">Código de verificación</p><p style=\"margin:0;color:#292344;font-family:Arial,sans-serif;font-size:30px;font-weight:700;letter-spacing:8px\">{}</p></div>",
                    escape_html(code)
                )
            })
            .unwrap_or_default();
        let action_html = self
            .action
            .map(|(url, label)| {
                format!(
                    "<div style=\"margin:30px 0 24px\"><a href=\"{}\" style=\"display:inline-block;padding:14px 24px;border-radius:9px;background:#5b4bdb;color:#ffffff;text-decoration:none;font-family:Arial,sans-serif;font-size:15px;font-weight:700\">{}</a></div>",
                    escape_html(url),
                    escape_html(label)
                )
            })
            .unwrap_or_default();
        let footer_note_html = self
            .footer_note
            .map(|note| {
                format!(
                    "<p style=\"margin:22px 0 0;padding-top:18px;border-top:1px solid #ecebf2;color:#737184;font-size:13px;line-height:1.6\">{}</p>",
                    escape_html(note)
                )
            })
            .unwrap_or_default();
        let html = render_template(
            HTML_TEMPLATE,
            &[
                ("PREHEADER", &escape_html(self.preheader)),
                ("DISPLAY_NAME", &escape_html(self.display_name)),
                ("TITLE", &escape_html(self.title)),
                ("MESSAGE", &escape_html(self.message)),
                ("VERIFICATION_CODE", &verification_code_html),
                ("ACTION", &action_html),
                ("FOOTER_NOTE", &footer_note_html),
            ],
        );
        let verification_code_text = self
            .verification_code
            .map(|code| format!("\n\nCódigo de verificación: {code}"))
            .unwrap_or_default();
        let action_text = self
            .action
            .map(|(url, label)| format!("\n\n{label}: {url}"))
            .unwrap_or_default();
        let footer_note_text = self
            .footer_note
            .map(|note| format!("\n\n{note}"))
            .unwrap_or_default();
        let text = format!(
            "XOVYRA\nCreación y gestión de contenido\n\n{}\n\nHola {},\n\n{}{}{}{}\n\nXOVYRA · Mensaje transaccional",
            self.title,
            self.display_name,
            self.message,
            verification_code_text,
            action_text,
            footer_note_text
        );
        (html, text)
    }
}

const HTML_TEMPLATE: &str = r#"<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="color-scheme" content="light">
    <title>{{TITLE}}</title>
  </head>
  <body style="margin:0;padding:0;background:#f4f5f9;color:#28263a;font-family:Arial,Helvetica,sans-serif">
    <div style="display:none;max-height:0;overflow:hidden;opacity:0;color:transparent">{{PREHEADER}}</div>
    <table role="presentation" width="100%" cellspacing="0" cellpadding="0" border="0" style="width:100%;background:#f4f5f9">
      <tr><td align="center" style="padding:36px 16px">
        <table role="presentation" width="600" cellspacing="0" cellpadding="0" border="0" style="width:100%;max-width:600px;border-collapse:separate;border-spacing:0">
          <tr><td style="padding:24px 32px;background:#211d3b;border-radius:16px 16px 0 0;border-bottom:3px solid #8877ff">
            <p style="margin:0;color:#ffffff;font-size:20px;font-weight:800;letter-spacing:3px">XOVYRA</p>
            <p style="margin:7px 0 0;color:#cbc7e4;font-size:11px;letter-spacing:1.4px;text-transform:uppercase">Creación y gestión de contenido</p>
          </td></tr>
          <tr><td style="padding:38px 36px 34px;background:#ffffff;border:1px solid #e8e8ef;border-top:0">
            <p style="margin:0 0 10px;color:#77738f;font-size:13px">Hola {{DISPLAY_NAME}},</p>
            <h1 style="margin:0 0 18px;color:#211d3b;font-size:27px;line-height:1.25;font-weight:700">{{TITLE}}</h1>
            <p style="margin:0;color:#555367;font-size:16px;line-height:1.7">{{MESSAGE}}</p>
            {{VERIFICATION_CODE}}
            {{ACTION}}
            {{FOOTER_NOTE}}
          </td></tr>
          <tr><td style="padding:20px 24px;background:#fafaff;border:1px solid #e8e8ef;border-top:0;border-radius:0 0 16px 16px;text-align:center">
            <p style="margin:0;color:#77758a;font-size:12px;line-height:1.6">Notificación transaccional enviada por XOVYRA.</p>
          </td></tr>
        </table>
      </td></tr>
    </table>
  </body>
</html>"#;

funcion render_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut rendered = String::with_capacity(template.len() + replacements.len() * 64);
    let mut remaining = template;
    while let Some(marker_start) = remaining.find("{{") {
        rendered.push_str(&remaining[..marker_start]);
        let marker_content = &remaining[marker_start + 2..];
        let Some(marker_end) = marker_content.find("}}") else {
            rendered.push_str(&remaining[marker_start..]);
            return rendered;
        };
        let marker = &marker_content[..marker_end];
        if let Some((_, value)) = replacements.iter().find(|(name, _)| *name == marker) {
            rendered.push_str(value);
        }
        remaining = &marker_content[marker_end + 2..];
    }
    rendered.push_str(remaining);
    rendered
}

funcion escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
