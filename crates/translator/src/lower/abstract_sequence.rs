//! The `length()` and `charAt(int)` of an abstract class that is a `CharSequence` through an
//! interface and declares neither. Its copy of `getChars` calls them, and the subclasses that
//! declare them implement them, as Java dispatches the copy's calls.

use super::object::CHAR_SEQUENCE_METHODS;
use super::*;

impl<'t> Lowering<'t> {
    /// Abstract method `i` of `CharSequence` (`length` or `charAt`, see `CHAR_SEQUENCE_METHODS`),
    /// declared in abstract class `t`, whose methods start at `first`: the subclasses' methods
    /// override it.
    pub(super) fn abstract_sequence_method(&self, t: usize, i: usize, first: usize) -> Method {
        let (name, params, ret) = CHAR_SEQUENCE_METHODS[i];
        Method {
            name: name.to_string(),
            params: params.iter().map(|&(name, ty)| Param { name: name.to_string(), ty }).collect(),
            ret: ret.unwrap_or(Type::class(CHAR_SEQUENCE)),
            body: Vec::new(),
            is_main: false,
            is_static: false,
            is_ctor: false,
            class: t,
            pos: self.types[t].node.pos(),
            is_abstract: true,
            is_synchronized: false,
            overrides: vec![first + i],
            type_params: Vec::new(),
            jdk_code: false,
        }
    }

    /// Links the `length()` and `charAt(int)` that a class declares below a `CharSequence` class
    /// to the interface's, so that the group of each contains them: a copy of `getChars` in an
    /// abstract superclass calls them on the object (see `default_get_chars`). The methods that
    /// the chain loop in `lower/object.rs` links are the top class's, so these are the rest.
    pub(super) fn link_sequence_subclasses(
        &self,
        methods: &mut Vec<Method>,
        first: usize,
        equals: usize,
    ) -> Result<(), Error> {
        for t in 0..self.types.len() {
            if self.types[t].node.kind() == "INTERFACE" || !self.is_char_sequence(t)? {
                continue;
            }
            for (i, (name, params, ret)) in CHAR_SEQUENCE_METHODS.into_iter().take(2).enumerate() {
                let found = methods.iter().take(equals).position(|m| {
                    m.class == t
                        && m.name == name
                        && !m.is_static
                        && !m.is_ctor
                        && m.params.len() == params.len()
                        && m.params.iter().zip(params).all(|(p, (_, ty))| p.ty == *ty)
                        && Some(m.ret) == ret
                });
                if let Some(m) = found
                    && !methods[m].overrides.contains(&(first + i))
                {
                    methods[m].overrides.push(first + i);
                }
            }
        }
        Ok(())
    }
}
