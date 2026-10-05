import Std

namespace Organization
variable {S A : Type} [DecidableEq S] [DecidableEq A]

def update (m : S → A → S) (s : S) (a : A) (observed : S) : S → A → S :=
  fun x b => if x = s ∧ b = a then observed else m x b

theorem update_witness (m : S → A → S) (s : S) (a : A) (observed : S) :
    update m s a observed s a = observed := by simp [update]

theorem update_preserves_other (m : S → A → S) (s x : S) (a b : A) (o : S)
    (h : ¬ (x = s ∧ b = a)) : update m s a o x b = m x b := by simp [update,h]

theorem truthful_update (m f : S → A → S) (s : S) (a : A)
    (h : ∀ x b, m x b = f x b) :
    ∀ x b, update m s a (f s a) x b = f x b := by
  intro x b
  by_cases e : x = s ∧ b = a
  · rcases e with ⟨rfl,rfl⟩; simp [update]
  · simp [update,e,h]

-- A finite fully witnessed table: the coverage premise is explicit.
omit [DecidableEq S] [DecidableEq A] in
theorem coverage_correct (m f : S → A → S)
    (covered : ∀ s a, ∃ witness, witness = f s a ∧ m s a = witness) :
    ∀ s a, m s a = f s a := by
  intro s a
  obtain ⟨w,hw,hm⟩ := covered s a
  exact hm.trans hw

def run (f : S → A → S) : S → List A → S
  | s, [] => s
  | s, a :: rest => run f (f s a) rest

omit [DecidableEq S] [DecidableEq A] in
theorem word_correspondence (m f : S → A → S)
    (h : ∀ s a, m s a = f s a) (word : List A) :
    ∀ s, run m s word = run f s word := by
  induction word with
  | nil => intro s; rfl
  | cons a rest ih => intro s; simp only [run,h]; exact ih _

-- Agreement only authenticates a primary checker if the other is healthy.
theorem checked_agreement (primary secondary : Bool) (truth : Bool)
    (healthy : secondary = truth) (agreed : primary = secondary) :
    primary = truth := agreed.trans healthy

theorem renewal_preserves_checker (old fresh : S → S → Bool)
    (copied_program : fresh = old)
    (old_healthy : ∀ p o, old p o = decide (p = o)) :
    ∀ p o, fresh p o = decide (p = o) := by
  subst fresh; exact old_healthy

-- Copying records cannot compensate for a changed interpretation.
omit [DecidableEq S] [DecidableEq A] in
theorem inheritance_correct (parent child physical : S → A → S)
    (parent_correct : ∀ s a, parent s a = physical s a)
    (transferred : ∀ s a, child s a = parent s a) :
    ∀ s a, child s a = physical s a := by
  intro s a; exact (transferred s a).trans (parent_correct s a)

theorem finite_budget (spent remaining budget : Nat)
    (conserved : spent + remaining = budget) : spent ≤ budget := by omega

theorem transfer_conservation (parent child moved : Nat) (h : moved ≤ parent) :
    (parent - moved) + (child + moved) = parent + child := by omega

-- A resident self-assertion is not a proof of correspondence.
theorem self_certification_counterexample :
    (true = true) ∧ ¬ ((0 : Nat) = 1) := by decide

end Organization
