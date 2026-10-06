-- Conditional information and cost lemmas; no Rust refinement or general
-- shortest-program algorithm is proved here. See the canonical formal account.
import Std

namespace RepresentationCosts

inductive Tree where
  | leaf : Tree
  | stem : Tree → Tree
  | fork : Tree → Tree → Tree
  deriving DecidableEq, Repr
open Tree

def size : Tree → Nat
  | leaf => 1
  | stem t => 1 + size t
  | fork t u => 1 + size t + size u

def cut : Nat → Tree → Tree
  | 0, _ => leaf
  | _ + 1, leaf => leaf
  | n + 1, stem t => stem (cut n t)
  | n + 1, fork t u => fork (cut n t) (cut n u)

def chain : Nat → Tree → Tree
  | 0, t => t
  | n + 1, t => stem (chain n t)

theorem size_chain (n : Nat) (t : Tree) : size (chain n t) = n + size t := by
  induction n with
  | zero => simp [chain]
  | succ n ih => simp [chain, size, ih]; omega

theorem bounded_probe_collision (n : Nat) :
    cut n (chain n leaf) = cut n (chain n (stem leaf)) := by
  induction n with
  | zero => rfl
  | succ n ih => simp [chain, cut, ih]

theorem bounded_probe_distinct (n : Nat) : chain n leaf ≠ chain n (stem leaf) := by
  intro h
  have hs := congrArg size h
  simp [size_chain, size] at hs

theorem recovery_injective {A B : Type} (q : A → B) (recover : B → A)
    (correct : ∀ t, recover (q t) = t) : ∀ {t u}, q t = q u → t = u := by
  intro t u h
  calc t = recover (q t) := (correct t).symm
       _ = recover (q u) := congrArg recover h
       _ = u := correct u

theorem bounded_probe_no_recovery (n : Nat) :
    ¬ ∃ recover : Tree → Tree, ∀ t, recover (cut n t) = t := by
  rintro ⟨recover, correct⟩
  exact bounded_probe_distinct n (recovery_injective (cut n) recover correct (bounded_probe_collision n))

theorem size_no_recovery : ¬ ∃ recover : Nat → Tree, ∀ t, recover (size t) = t := by
  rintro ⟨recover, correct⟩
  have h := recovery_injective size recover correct (t := stem (stem leaf)) (u := fork leaf leaf) (by rfl)
  cases h

def arity : Tree → Nat
  | leaf => 0
  | stem _ => 1
  | fork _ _ => 2

def probe : Tree → List Bool → Option Nat
  | t, [] => some (arity t)
  | leaf, _ :: _ => none
  | stem t, false :: p => probe t p
  | stem _, true :: _ => none
  | fork t _, false :: p => probe t p
  | fork _ u, true :: p => probe u p

theorem all_paths_separate (t u : Tree) (h : ∀ p, probe t p = probe u p) : t = u := by
  induction t generalizing u with
  | leaf =>
    cases u with
    | leaf => rfl
    | stem _ => have := h []; simp [probe, arity] at this
    | fork _ _ => have := h []; simp [probe, arity] at this
  | stem t ih =>
    cases u with
    | leaf => have := h []; simp [probe, arity] at this
    | stem u => exact congrArg stem (ih u (fun p => h (false :: p)))
    | fork _ _ => have := h []; simp [probe, arity] at this
  | fork t v ih₁ ih₂ =>
    cases u with
    | leaf => have := h []; simp [probe, arity] at this
    | stem _ => have := h []; simp [probe, arity] at this
    | fork u w =>
      have h₁ := ih₁ u (fun p => h (false :: p))
      have h₂ := ih₂ w (fun p => h (true :: p))
      simp [h₁, h₂]

theorem equality_queries_separate {A : Type} (t u : A)
    (h : ∀ v, (t = v ↔ u = v)) : t = u := (h t).mp rfl |>.symm

-- A cost transfer theorem with explicit code and dyadic-work overhead.
theorem transfer_bound (l l' w w' c d k b : Nat)
    (code : l' ≤ l + c) (work : w' + 1 ≤ 2^d * (w+1)^k)
    (budget : w + 1 ≤ 2^b) :
    l' + (d + k*b) ≤ l + c + d + k*b ∧ w' + 1 ≤ 2^(d+k*b) := by
  constructor
  · omega
  · calc
      w' + 1 ≤ 2^d * (w+1)^k := work
      _ ≤ 2^d * (2^b)^k := Nat.mul_le_mul_left _ (Nat.pow_le_pow_left budget k)
      _ = 2^(d+k*b) := by rw [← Nat.pow_mul, ← Nat.pow_add]; rw [Nat.mul_comm b k]

-- Complete coverage of a finite sublevel certifies a global winner.
theorem global_from_sublevel {P V : Type} (cost : P → Nat) (eval : P → Option V)
    (candidates : List P) (B : Nat) (p : P) (v : V)
    (witness : eval p = some v) (bound : cost p ≤ B)
    (coverage : ∀ q, cost q ≤ B → q ∈ candidates)
    (least : ∀ q ∈ candidates, eval q = some v → cost p ≤ cost q) :
    eval p = some v ∧ ∀ q, eval q = some v → cost p ≤ cost q := by
  refine ⟨witness, ?_⟩
  intro q hq
  by_cases hb : cost q ≤ B
  · exact least q (coverage q hb) hq
  · omega

theorem dominance_pruning {P V : Type} (cost : P → Nat) (eval : P → Option V)
    (full kept : List P) (p : P) (v : V)
    (dominates : ∀ q ∈ full, ∃ r ∈ kept, eval r = eval q ∧ cost r ≤ cost q)
    (least : ∀ r ∈ kept, eval r = some v → cost p ≤ cost r) :
    ∀ q ∈ full, eval q = some v → cost p ≤ cost q := by
  intro q hq value
  obtain ⟨r, hr, equivalent, cheaper⟩ := dominates q hq
  have h := least r hr (equivalent.trans value)
  omega

theorem scaled_transfer_bound (l l' w w' a c d k b : Nat)
    (code : l' ≤ a*l + c) (work : w' + 1 ≤ 2^d * (w+1)^k)
    (budget : w + 1 ≤ 2^b) :
    l' + (d + k*b) ≤ a*l + c + d + k*b ∧ w' + 1 ≤ 2^(d+k*b) :=
  transfer_bound (a*l) l' w w' c d k b code work budget

theorem batch_dominance {A : Type} (xs : List A) (f g : A → Nat)
    (pointwise : ∀ t, f t ≤ g t) (c d : Nat) (base : c ≤ d) :
    c + (xs.map f).sum ≤ d + (xs.map g).sum := by
  have h : (xs.map f).sum ≤ (xs.map g).sum := by
    induction xs with
    | nil => simp
    | cons t ts ih => simp only [List.map_cons, List.sum_cons]; have := pointwise t; omega
  omega

theorem amortization_break_even (c p overhead saving copies : Nat) :
    c + copies * (p + saving) ≥ (c + overhead) + copies * p ↔
    overhead ≤ copies * saving := by
  rw [Nat.mul_add]
  omega

#print axioms size_chain
#print axioms bounded_probe_collision
#print axioms bounded_probe_distinct
#print axioms recovery_injective
#print axioms bounded_probe_no_recovery
#print axioms size_no_recovery
#print axioms all_paths_separate
#print axioms equality_queries_separate
#print axioms transfer_bound
#print axioms global_from_sublevel
#print axioms dominance_pruning
#print axioms scaled_transfer_bound
#print axioms batch_dominance
#print axioms amortization_break_even
end RepresentationCosts
