use super::*;
use crate::{
    hir::{Literal, Value},
    lir::{Block, BlockId},
};
fn value(index: usize) -> Value {
    Value {
        id: ValueId::Local(index),
        ty: "Int".into(),
    }
}
fn root() -> Operation {
    // The caller checks the getter origin; this models its ownership token.
    Operation::Literal {
        result: value(0),
        value: Literal::Int(1),
    }
}
fn alias(index: usize, source: usize) -> Operation {
    Operation::Declare {
        result: value(index),
        initial: Some(value(source)),
    }
}
fn drop(ids: &[usize]) -> Operation {
    Operation::Release(ids.iter().map(|i| value(*i).id).collect())
}
fn branch(yes: usize, no: usize) -> Terminator {
    Terminator::Branch {
        condition: Value {
            id: ValueId::Local(9),
            ty: "Bool".into(),
        },
        yes: BlockId(yes),
        no: BlockId(no),
    }
}
fn done(ids: &[usize]) -> Terminator {
    Terminator::Return {
        value: None,
        cleanup: ids.iter().map(|i| value(*i).id).collect(),
        span: None,
    }
}
fn solve(blocks: Vec<Block>, budget: &mut usize) -> Option<BTreeSet<Site>> {
    let members = blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|op| match op {
            Operation::Literal { result, .. } | Operation::Declare { result, .. } => {
                Some(result.id.clone())
            }
            _ => None,
        })
        .collect();
    releases(
        &Function {
            blocks,
            slots: vec![],
        },
        &members,
        &value(0).id,
        budget,
    )
}
#[test]
fn last_release_uses_each_return_path_and_original_cleanup_order() {
    let result = solve(
        vec![
            Block {
                instructions: vec![root(), alias(1, 0), drop(&[0])],
                terminator: branch(1, 2),
            },
            Block {
                instructions: vec![alias(2, 1), drop(&[1, 2])],
                terminator: done(&[]),
            },
            Block {
                instructions: vec![],
                terminator: done(&[9, 1]),
            },
        ],
        &mut 1000,
    )
    .unwrap();
    assert_eq!(result, BTreeSet::from([(1, 1, 1), (2, 0, 1)]));
}
#[test]
fn incompatible_join_states_keep_ordinary_ownership() {
    assert!(
        solve(
            vec![
                Block {
                    instructions: vec![root(), alias(1, 0), drop(&[0])],
                    terminator: branch(1, 2)
                },
                Block {
                    instructions: vec![drop(&[1])],
                    terminator: Terminator::Jump(BlockId(3))
                },
                Block {
                    instructions: vec![],
                    terminator: Terminator::Jump(BlockId(3))
                },
                Block {
                    instructions: vec![],
                    terminator: done(&[1])
                },
            ],
            &mut 1000
        )
        .is_none()
    );
}
#[test]
fn loop_redefinition_requires_old_aliases_to_have_died() {
    assert!(
        solve(
            vec![
                Block {
                    instructions: vec![],
                    terminator: Terminator::Jump(BlockId(1))
                },
                Block {
                    instructions: vec![root(), alias(1, 0), drop(&[0])],
                    terminator: branch(1, 2)
                },
                Block {
                    instructions: vec![],
                    terminator: done(&[1])
                },
            ],
            &mut 1000
        )
        .is_none()
    );
    let result = solve(
        vec![
            Block {
                instructions: vec![],
                terminator: Terminator::Jump(BlockId(1)),
            },
            Block {
                instructions: vec![root(), alias(1, 0), drop(&[0, 1])],
                terminator: branch(1, 2),
            },
            Block {
                instructions: vec![],
                terminator: done(&[]),
            },
        ],
        &mut 1000,
    )
    .unwrap();
    assert_eq!(result, BTreeSet::from([(1, 2, 1)]));
}
#[test]
fn exhausted_flow_budget_disables_borrowing() {
    for mut budget in [0, 7] {
        assert!(
            solve(
                vec![
                    Block {
                        instructions: vec![root(), alias(1, 0)],
                        terminator: Terminator::Jump(BlockId(1))
                    },
                    Block {
                        instructions: vec![],
                        terminator: done(&[0, 1])
                    },
                ],
                &mut budget
            )
            .is_none()
        );
        assert_eq!(budget, 0);
    }
}
