use tera;

pub struct View {
    pub tera: tera::Tera
}

impl View {
    pub fn render_index(&self) -> tera::Result<String> {
        let mut context = tera::Context::new();
        context.insert("name", "ИмяПользователя");
        return self.tera.render("index.html", &context);
    }
}
