import Mathlib.Analysis.SpecificLimits.Basic
import Mathlib.Tactic

open Filter Topology
namespace T2Review

/-- Convergence of an actually forced recurrence, with no pre-assumed endpoint limit. -/
theorem forced_contraction (e u : ℕ → ℝ) (q : ℝ)
    (positive : ∀ n, 0 ≤ e n) (hq : 0 ≤ q) (hq1 : q < 1)
    (step : ∀ n, e (n+1) ≤ q * e n + u n)
    (vanishes : Tendsto u atTop (𝓝 0)) : Tendsto e atTop (𝓝 0) := by
  apply tendsto_order.mpr
  constructor
  · intro a ha
    exact Eventually.of_forall fun n => ha.trans_le (positive n)
  · intro eps heps
    have hsmall : 0 < (1-q)*(eps/2) := mul_pos (sub_pos.mpr hq1) (by linarith)
    obtain ⟨N, hN⟩ := eventually_atTop.mp (vanishes.eventually (gt_mem_nhds hsmall))
    have bound : ∀ m, e (N+m) ≤ q^m * e N + eps/2 := by
      intro m
      induction m with
      | zero => simp; linarith
      | succ m ih =>
        have hs := step (N+m)
        have hu := hN (N+m) (by omega)
        have hi := mul_le_mul_of_nonneg_left ih hq
        rw [pow_succ]
        convert (show e (N+m+1) ≤ q^m*q*e N+eps/2 by nlinarith) using 1
    have geo : Tendsto (fun m : ℕ => q^m * e N) atTop (𝓝 0) := by
      simpa using (tendsto_pow_atTop_nhds_zero_of_lt_one hq hq1).mul_const (e N)
    obtain ⟨M, hM⟩ := eventually_atTop.mp (geo.eventually (gt_mem_nhds (show 0 < eps/2 by linarith)))
    apply eventually_atTop.mpr
    refine ⟨N+M, ?_⟩
    intro n hn
    have hm : M ≤ n-N := by omega
    have hb := bound (n-N)
    have hg := hM (n-N) hm
    rw [Nat.add_sub_of_le (by omega : N ≤ n)] at hb
    linarith

/-- Uniform control of ALL intermediate phases, then lifting along any diverging
local cycle clock. The phase selector is unrestricted and may depend on time. -/
theorem local_cycle_convergence {Phase : Type*}
    (e u w : ℕ → ℝ) (error : ℕ → Phase → ℝ) (q C : ℝ)
    (positive : ∀ n, 0 ≤ e n) (hq : 0 ≤ q) (hq1 : q < 1)
    (step : ∀ n, e (n+1) ≤ q * e n + u n)
    (hu : Tendsto u atTop (𝓝 0)) (hw : Tendsto w atTop (𝓝 0))
    (phase_positive : ∀ k p, 0 ≤ error k p)
    (phase_bound : ∀ k p, error k p ≤ C*e k + w k)
    (cycle : ℕ → ℕ) (phase : ℕ → Phase)
    (clock : Tendsto cycle atTop atTop) :
    Tendsto (fun t => error (cycle t) (phase t)) atTop (𝓝 0) := by
  have ends := forced_contraction e u q positive hq hq1 step hu
  have upper : Tendsto (fun k => C*e k+w k) atTop (𝓝 0) := by
    simpa using (ends.const_mul C).add hw
  exact squeeze_zero (fun t => phase_positive _ _)
    (fun t => phase_bound _ _) (upper.comp clock)

/-- Well-founded block composition: predecessor convergence entails both forcing
limits. This abstracts, rather than assumes a particular network scheduler. -/
theorem block_composition {Block Phase : Type*} (r : Block → Block → Prop)
    (wf : WellFounded r)
    (e u w : Block → ℕ → ℝ) (error : Block → ℕ → Phase → ℝ)
    (q C : Block → ℝ) (cycle : Block → ℕ → ℕ) (phase : Block → ℕ → Phase)
    (positive : ∀ b n, 0 ≤ e b n) (hq : ∀ b, 0 ≤ q b ∧ q b < 1)
    (step : ∀ b n, e b (n+1) ≤ q b * e b n + u b n)
    (phase_positive : ∀ b k p, 0 ≤ error b k p)
    (phase_bound : ∀ b k p, error b k p ≤ C b*e b k+w b k)
    (clock : ∀ b, Tendsto (cycle b) atTop atTop)
    (forcing : ∀ b, (∀ a, r a b →
      Tendsto (fun t => error a (cycle a t) (phase a t)) atTop (𝓝 0)) →
      Tendsto (u b) atTop (𝓝 0) ∧ Tendsto (w b) atTop (𝓝 0)) :
    ∀ b, Tendsto (fun t => error b (cycle b t) (phase b t)) atTop (𝓝 0) := by
  intro b
  induction b using wf.induction with
  | h b ih =>
    obtain ⟨hu, hw⟩ := forcing b ih
    exact local_cycle_convergence (e b) (u b) (w b) (error b) (q b) (C b)
      (positive b) (hq b).1 (hq b).2 (step b) hu hw (phase_positive b)
      (phase_bound b) (cycle b) (phase b) (clock b)

#print axioms forced_contraction
#print axioms local_cycle_convergence
#print axioms block_composition
end T2Review
