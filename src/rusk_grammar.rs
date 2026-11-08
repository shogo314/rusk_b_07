use crate::rusk_grammar_trait::{Rusk, RuskGrammarTrait};
#[derive(Default)]
pub struct RuskGrammar<'t> {
    pub root: Option<Rusk<'t>>,
}
impl<'t> RuskGrammarTrait<'t> for RuskGrammar<'t> {
    fn rusk(&mut self, arg: &Rusk<'t>) -> parol_runtime::Result<()> {
        self.root = Some(arg.clone());
        Ok(())
    }
}
