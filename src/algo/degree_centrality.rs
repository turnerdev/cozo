use std::collections::BTreeMap;

use anyhow::ensure;

use crate::algo::AlgoImpl;
use crate::data::expr::Expr;
use crate::data::program::{MagicAlgoRuleArg, MagicSymbol};
use crate::data::symb::Symbol;
use crate::data::tuple::Tuple;
use crate::data::value::DataValue;
use crate::runtime::derived::DerivedRelStore;
use crate::runtime::transact::SessionTx;

pub(crate) struct DegreeCentrality;

impl AlgoImpl for DegreeCentrality {
    fn name(&self) -> Symbol {
        Symbol::from("degree_centrality")
    }

    fn arity(&self) -> usize {
        4
    }

    fn run(
        &self,
        tx: &mut SessionTx,
        rels: &[MagicAlgoRuleArg],
        _opts: &BTreeMap<Symbol, Expr>,
        stores: &BTreeMap<MagicSymbol, DerivedRelStore>,
        out: &DerivedRelStore,
    ) -> anyhow::Result<()> {
        ensure!(
            rels.len() == 1,
            "'degree_centrality' requires a single input relation, got {}",
            rels.len()
        );
        let it = rels[0].iter(tx, stores)?;
        let mut counter: BTreeMap<DataValue, (usize, usize, usize)> = BTreeMap::new();
        for tuple in it {
            let tuple = tuple?;
            ensure!(
                tuple.0.len() >= 2,
                "'degree_centrality' requires input relation to be a tuple of two elements"
            );
            let from = tuple.0[0].clone();
            let (from_total, from_out, _) = counter.entry(from).or_default();
            *from_total += 1;
            *from_out += 1;

            let to = tuple.0[1].clone();
            let (to_total, _, to_in) = counter.entry(to).or_default();
            *to_total += 1;
            *to_in += 1;
        }
        for (k, (total_d, out_d, in_d)) in counter.into_iter() {
            let tuple = Tuple(vec![
                k,
                DataValue::from(total_d as i64),
                DataValue::from(out_d as i64),
                DataValue::from(in_d as i64),
            ]);
            out.put(tuple, 0);
        }
        Ok(())
    }
}