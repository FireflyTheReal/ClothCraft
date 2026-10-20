//! Exact memoization for one fixed residual function at one nonlinear iterate.
//! The owner must discard this cache whenever rest, target, forces or controls
//! change. No samples are interpolated and no approximate keys are used.
use glam::DVec3;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    time::Instant,
};

pub struct Samples {
    enabled: bool,
    values: RefCell<HashMap<Vec<u64>, Vec<f64>>>,
    pub calls: Cell<usize>,
    pub evaluations: Cell<usize>,
    pub evaluation_ns: Cell<u128>,
}

impl Samples {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            values: RefCell::new(HashMap::new()),
            calls: Cell::new(0),
            evaluations: Cell::new(0),
            evaluation_ns: Cell::new(0),
        }
    }

    pub fn evaluate(
        &self,
        x: &[DVec3],
        reactions: &[f64],
        compute: impl FnOnce() -> Result<Vec<f64>, String>,
    ) -> Result<Vec<f64>, String> {
        self.calls.set(self.calls.get() + 1);
        let key = self.enabled.then(|| {
            let mut key = Vec::with_capacity(2 + 3 * x.len() + reactions.len());
            key.extend([x.len() as u64, reactions.len() as u64]);
            for p in x {
                key.extend(p.to_array().map(f64::to_bits));
            }
            key.extend(reactions.iter().map(|v| v.to_bits()));
            key
        });
        if let Some(key) = &key
            && let Some(value) = self.values.borrow().get(key)
        {
            return Ok(value.clone());
        }
        self.evaluations.set(self.evaluations.get() + 1);
        let start = Instant::now();
        let result = compute();
        self.evaluation_ns
            .set(self.evaluation_ns.get() + start.elapsed().as_nanos());
        let value = result?;
        if let Some(key) = key {
            // An unexpected larger experiment still computes exactly; it merely
            // stops retaining new samples. This is not a contact candidate cap.
            if self.values.borrow().len() < 1024 {
                self.values.borrow_mut().insert(key, value.clone());
            }
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_samples_are_reused_without_changing_signed_zero_or_nan_payloads() {
        let s = Samples::new(true);
        let x = [DVec3::ZERO];
        let result = vec![-0., f64::from_bits(0x7ff8000000000023)];
        s.evaluate(&x, &[1.], || Ok(result.clone())).unwrap();
        let reused = s
            .evaluate(&x, &[1.], || panic!("duplicate evaluation"))
            .unwrap();
        assert_eq!(
            reused.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            result.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        for (p, l) in [
            (DVec3::new(-0., 0., 0.), 1.),
            (DVec3::X, 1.),
            (DVec3::ZERO, 2.),
        ] {
            s.evaluate(&[p], &[l], || Ok(vec![3.])).unwrap();
        }
        assert_eq!((s.calls.get(), s.evaluations.get()), (5, 4));
    }

    #[test]
    fn errors_disabled_mode_and_new_iterations_do_not_reuse_results() {
        for enabled in [false, true] {
            let s = Samples::new(enabled);
            assert!(s.evaluate(&[], &[], || Err("failure".into())).is_err());
            assert_eq!(s.evaluate(&[], &[], || Ok(vec![4.])).unwrap(), vec![4.]);
            assert_eq!(s.evaluations.get(), 2);
        }
        let s = Samples::new(false);
        for _ in 0..2 {
            s.evaluate(&[], &[], || Ok(vec![4.])).unwrap();
        }
        assert_eq!(s.evaluations.get(), 2);
        let fresh = Samples::new(true);
        assert_eq!(fresh.evaluate(&[], &[], || Ok(vec![7.])).unwrap(), vec![7.]);
        assert_eq!(fresh.evaluations.get(), 1);
    }

    #[test]
    fn storage_limit_falls_back_to_exact_evaluation() {
        let s = Samples::new(true);
        for i in 0..1030 {
            let r = [i as f64];
            assert_eq!(s.evaluate(&[], &r, || Ok(r.to_vec())).unwrap(), r);
        }
        assert_eq!(s.values.borrow().len(), 1024);
        s.evaluate(&[], &[0.], || panic!("retained sample"))
            .unwrap();
        s.evaluate(&[], &[1029.], || Ok(vec![1029.])).unwrap();
        assert_eq!(s.evaluations.get(), 1031);
    }
}
