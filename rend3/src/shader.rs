//! Holds the shader processing infrastructure for all shaders.

use std::collections::{HashMap, HashSet};

use handlebars::{Context, Handlebars, Helper, HelperDef, Output, RenderContext, RenderError};
use parking_lot::Mutex;
use rust_embed::RustEmbed;
use serde::Serialize;

use crate::RendererProfile;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/shaders"]
struct Rend3ShaderSources;

#[derive(Debug, Default, Serialize)]
pub struct ShaderConfig {
    pub profile: Option<RendererProfile>,
}

pub struct ShaderPreProcessor {
    files: HashMap<String, String>,
}

impl ShaderPreProcessor {
    pub fn new() -> Self {
        let mut v = Self { files: HashMap::new() };
        v.add_shaders_embed::<Rend3ShaderSources>("rend3");
        v
    }

    pub fn add_shaders_embed<T: RustEmbed>(&mut self, prefix: &str) {
        for file in T::iter() {
            let contents = String::from_utf8(T::get(&file).unwrap().data.into_owned()).unwrap();
            self.files.insert(format!("{prefix}/{file}"), contents);
        }
    }

    pub fn add_shader(&mut self, name: &str, contents: &str) {
        self.files.insert(name.to_owned(), contents.to_owned());
    }

    pub fn files(&self) -> std::collections::hash_map::Keys<'_, String, String> {
        self.files.keys()
    }

    pub fn get(&self, name: &str) -> Option<&String> {
        self.files.get(name)
    }

    pub fn render_shader<T: Serialize>(&self, base: &str, config: &T) -> Result<String, RenderError> {
        let mut include_status = Mutex::new(HashSet::new());
        include_status.get_mut().insert(base.to_string());

        let mut registry = Handlebars::new();
        registry.set_strict_mode(true);
        registry.set_dev_mode(cfg!(debug_assertions));
        registry.register_escape_fn(handlebars::no_escape);
        registry.register_helper("include", Box::new(ShaderIncluder::new(base, &self.files)));
        let contents = self.files.get(base).ok_or_else(|| {
            RenderError::new(format!(
                "Base shader {base} is not registered. All registered shaders: {}",
                registered_shader_string(&self.files)
            ))
        })?;

        registry.render_template(contents, config)
    }
}

impl Default for ShaderPreProcessor {
    fn default() -> Self {
        Self::new()
    }
}

fn registered_shader_string(files: &HashMap<String, String>) -> String {
    let mut v: Vec<_> = files.keys().cloned().collect();
    v.sort_unstable();
    v.join(", ")
}

struct ShaderIncluder<'a> {
    files: &'a HashMap<String, String>,
    include_state: Mutex<HashSet<String>>,
}
impl<'a> ShaderIncluder<'a> {
    fn new(base: &str, files: &'a HashMap<String, String>) -> Self {
        Self {
            files,
            include_state: Mutex::new({
                let mut set = HashSet::new();
                set.insert(base.to_owned());
                set
            }),
        }
    }
}
impl<'a> HelperDef for ShaderIncluder<'a> {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        r: &'reg Handlebars<'reg>,
        _ctx: &'rc Context,
        _rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> handlebars::HelperResult {
        let file_name_value = h
            .param(0)
            .ok_or_else(|| RenderError::new("include helper must have a single argument for the include path"))?
            .value();
        let file_name = match file_name_value {
            handlebars::JsonValue::String(s) => s,
            _ => return Err(RenderError::new("include helper's first argument must be a string")),
        };

        let mut include_status = self.include_state.try_lock().unwrap();
        if include_status.contains(file_name) {
            return Ok(());
        }
        include_status.insert(file_name.clone());
        drop(include_status);

        let contents = self.files.get(file_name).ok_or_else(|| {
            RenderError::new(format!(
                "Included file \"{file_name}\" is not registered. All registered files: {}",
                registered_shader_string(self.files)
            ))
        })?;

        out.write(&r.render_template(contents, &())?)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{ShaderConfig, ShaderPreProcessor};

    #[test]
    fn simple_include() {
        let mut pp = ShaderPreProcessor::new();
        pp.add_shader("simple", "{{include \"other\"}} simple");
        pp.add_shader("other", "other");
        let config = ShaderConfig { profile: None };
        let output = pp.render_shader("simple", &config).unwrap();

        assert_eq!(output, "other simple");
    }

    #[test]
    fn recursive_include() {
        let mut pp = ShaderPreProcessor::new();
        pp.add_shader("simple", "{{include \"other\"}} simple");
        pp.add_shader("other", "{{include \"simple\"}} other");
        let config = ShaderConfig { profile: None };
        let output = pp.render_shader("simple", &config).unwrap();

        assert_eq!(output, " other simple");
    }

    #[test]
    fn error_include() {
        let mut pp = ShaderPreProcessor::new();
        pp.add_shader("simple", "{{include \"other\"}} simple");
        let config = ShaderConfig { profile: None };
        let output = pp.render_shader("simple", &config);

        assert!(output.is_err(), "Expected error, got {output:?}");
    }

    #[test]
    fn no_arg_include() {
        let mut pp = ShaderPreProcessor::new();
        pp.add_shader("simple", "{{include}} simple");
        let config = ShaderConfig { profile: None };
        let output = pp.render_shader("simple", &config);

        assert!(output.is_err(), "Expected error, got {output:?}");
    }
}
