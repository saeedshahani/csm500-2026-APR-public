import Std

/- Abstract arithmetic for the opt-in workflow scheduler. This file does not
   formalise graph decoding, matching, native admission or the Rust implementation. -/
namespace WorkflowContracts

def potential (burst work credit : Nat) : Nat := (burst + 1) * work + credit - burst

theorem processing_decreases (burst before after credit : Nat)
    (decrease : after < before) :
    potential burst after burst < potential burst before credit := by
  have h := Nat.mul_le_mul_left (burst + 1) (show after + 1 ≤ before by omega)
  simp only [Nat.mul_add, Nat.mul_one] at h
  simp only [potential]
  omega

theorem administration_decreases (burst work credit : Nat) (available : 0 < credit)
    (unfinished : 0 < work) :
    potential burst work (credit - 1) < potential burst work credit := by
  have h := Nat.mul_le_mul_left (burst + 1) unfinished
  simp only [Nat.mul_one] at h
  simp only [potential]
  omega

theorem initial_bound (burst work : Nat) :
    potential burst work burst = (burst + 1) * work := by
  simp [potential]

theorem sum_append (xs ys : List Nat) : (xs ++ ys).sum = xs.sum + ys.sum := by
  induction xs with
  | nil => simp
  | cons x xs ih => simp [ih, Nat.add_assoc]

theorem branching_decreases (pre post children : List Nat) (old next : Nat)
    (contract : next + children.sum < old) :
    (pre ++ next :: (children ++ post)).sum < (pre ++ old :: post).sum := by
  simp only [sum_append, List.sum_cons]
  omega

theorem record_nonincrease (r : Nat → Nat) (h : ∀ n, r (n + 1) ≤ r n)
    (n k : Nat) : r (n + k) ≤ r n := by
  induction k with
  | zero => simp
  | succ k ih =>
    have next := h (n + k)
    have eq : n + (k + 1) = n + k + 1 := by omega
    rw [eq]; omega

theorem no_infinite_descent (r : Nat → Nat) (h : ∀ n, r (n + 1) < r n) : False := by
  have bound : ∀ n, r n + n ≤ r 0 := by
    intro n; induction n with
    | zero => simp
    | succ n ih => have step := h n; omega
  have impossible := bound (r 0 + 1)
  omega

-- The conditional action-fairness result permits arbitrary neutral intervals.
theorem fair_recovery (r : Nat → Nat) (enabled occurs : Nat → Prop)
    (mono : ∀ n, r (n + 1) ≤ r n)
    (available : ∀ n, r n ≠ 0 → enabled n)
    (decrease : ∀ n, occurs n → r (n + 1) < r n)
    (fair : (∃ n, ∀ m, n ≤ m → enabled m) → ∀ n, ∃ m, n ≤ m ∧ occurs m) :
    ∃ n, r n = 0 := by
  apply Classical.byContradiction
  intro never
  have nz : ∀ n, r n ≠ 0 := fun n eq => never ⟨n, eq⟩
  have events := fair ⟨0, fun n _ => available n (nz n)⟩
  have impossible : ∀ k, ∀ n, r n = k → False := by
    intro k; induction k using Nat.strongRecOn with
    | ind k ih =>
      intro n eq
      obtain ⟨m, le, event⟩ := events n
      have bound := record_nonincrease r mono n (m - n)
      have same : n + (m - n) = m := by omega
      rw [same] at bound
      have strict := decrease m event
      exact ih (r (m + 1)) (by omega) (m + 1) rfl
  exact impossible (r 0) 0 rfl

example : (0 : Nat) + [2, 2].sum < 5 := by decide
example : potential 2 4 2 < potential 2 5 0 := by decide

#print axioms processing_decreases
#print axioms administration_decreases
#print axioms initial_bound
#print axioms branching_decreases
#print axioms record_nonincrease
#print axioms no_infinite_descent
#print axioms fair_recovery
end WorkflowContracts
