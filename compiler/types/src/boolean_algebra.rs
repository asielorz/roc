use self::Bool::*;
use crate::subs::{Content, FlatType, Subs, Variable};
use roc_collections::all::SendSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Bool {
    Shared,
    Container(Variable, SendSet<Variable>),
}

pub fn var_is_shared(subs: &Subs, var: Variable) -> bool {
    match subs.get_without_compacting(var).content {
        Content::Structure(FlatType::Boolean(Bool::Shared)) => true,
        _ => false,
    }
}

// pull all of the "nested" variables into one container
pub fn flatten(subs: &mut Subs, var: Variable) {
    match subs.get_without_compacting(var).content {
        Content::Structure(FlatType::Boolean(Bool::Container(cvar, mvars))) => {
            let flattened_mvars = var_to_variables(subs, cvar, mvars);

            println!(
                "for {:?}, cvar={:?} and all mvars are {:?}",
                var, cvar, flattened_mvars
            );

            let content =
                Content::Structure(FlatType::Boolean(Bool::Container(cvar, flattened_mvars)));

            subs.set_content(var, content);
        }
        Content::Structure(FlatType::Boolean(Bool::Shared)) => {
            // do nothing
        }
        _ => {
            // do nothing
        }
    }
}

fn var_to_variables(
    subs: &Subs,
    cvar: Variable,
    start_vars: SendSet<Variable>,
) -> SendSet<Variable> {
    let mut stack: Vec<_> = start_vars.into_iter().collect();
    let mut seen = SendSet::default();
    seen.insert(cvar);
    let mut result = SendSet::default();

    while let Some(var) = stack.pop() {
        if seen.contains(&var) {
            continue;
        }

        seen.insert(var);

        match subs.get_without_compacting(var).content {
            Content::Structure(FlatType::Boolean(Bool::Container(cvar, mvars))) => {
                let it = std::iter::once(cvar).chain(mvars.into_iter());

                for v in it {
                    if !seen.contains(&v) {
                        stack.push(v);
                    }
                }
            }
            Content::Structure(FlatType::Boolean(Bool::Shared)) => {
                // do nothing
            }
            _other => {
                println!("add to result: {:?} at {:?} ", var, _other);
                result.insert(var);
            }
        }
    }

    result
}

impl Bool {
    pub fn shared() -> Self {
        Bool::Shared
    }

    pub fn container<I>(cvar: Variable, mvars: I) -> Self
    where
        I: IntoIterator<Item = Variable>,
    {
        Bool::Container(cvar, mvars.into_iter().collect())
    }

    pub fn variable(var: Variable) -> Self {
        Bool::Container(var, SendSet::default())
    }

    pub fn is_fully_simplified(&self, subs: &Subs) -> bool {
        match self {
            Shared => true,
            Container(cvar, mvars) => {
                !var_is_shared(subs, *cvar)
                    && !(mvars.iter().any(|mvar| var_is_shared(subs, *mvar)))
            }
        }
    }

    pub fn is_unique(&self, subs: &Subs) -> bool {
        debug_assert!(self.is_fully_simplified(subs));

        match self {
            Shared => false,
            _ => true,
        }
    }

    pub fn variables(&self) -> SendSet<Variable> {
        match self {
            Shared => SendSet::default(),
            Container(cvar, mvars) => {
                let mut mvars = mvars.clone();
                mvars.insert(*cvar);

                mvars
            }
        }
    }

    pub fn map_variables<F>(&self, f: &mut F) -> Self
    where
        F: FnMut(Variable) -> Variable,
    {
        match self {
            Bool::Shared => Bool::Shared,
            Bool::Container(cvar, mvars) => {
                let new_cvar = f(*cvar);
                let new_mvars = mvars.iter().map(|var| f(*var)).collect();

                Bool::Container(new_cvar, new_mvars)
            }
        }
    }
}
