-- Conservative conjunction/assignment learning: abstraction of SAM (Juba, Le and Stern, 2021).
-- These lemmas do not verify Rust, grounding, environmental stationarity or evidence collection.
import Std
namespace SafeLearning
abbrev State (I : Type) := I → Bool
structure Action (I : Type) where
  pre : I → Option Bool
  eff : I → Option Bool

def Applicable (a : Action I) (s : State I) : Prop :=
  ∀ i v, a.pre i = some v → s i = v

def apply (a : Action I) (s : State I) : State I :=
  fun i => (a.eff i).getD (s i)

variable {I D : Type} (actual learned : Action I) (before after : D → State I)
variable (valid : ∀ d, Applicable actual (before d))
variable (transition : ∀ d, after d = apply actual (before d))
variable (common : ∀ i v, learned.pre i = some v ↔ ∀ d, before d i = v)
variable (changes : ∀ i v, learned.eff i = some v ↔
  ∃ d, before d i ≠ after d i ∧ after d i = v)

include before valid common in
theorem pre_sound : ∀ i v, actual.pre i = some v → learned.pre i = some v := by
  intro i v h
  apply (common i v).mpr
  intro d
  exact valid d i v h

include before after transition changes in
theorem effect_sound : ∀ i v, learned.eff i = some v → actual.eff i = some v := by
  intro i v h
  obtain ⟨d,changed,value⟩ := (changes i v).mp h
  have trans := congrFun (transition d) i
  cases he : actual.eff i with
  | none =>
    simp [apply, he] at trans
    exact False.elim (changed trans.symm)
  | some w =>
    simp [apply, he] at trans
    have eq : w = v := trans.symm.trans value
    rw [eq]

include before after transition common changes in
theorem missing_effect_is_precondition (i : I) (v : Bool)
    (effect : actual.eff i = some v) (missing : learned.eff i = none) :
    learned.pre i = some v := by
  apply (common i v).mpr
  intro d
  have trans := congrFun (transition d) i
  simp [apply, effect] at trans
  by_cases same : before d i = v
  · exact same
  · have different : before d i ≠ after d i := by
      intro eq
      exact same (eq.trans trans)
    have observed := (changes i v).mpr ⟨d,different,trans⟩
    rw [missing] at observed
    cases observed

include before after valid transition common changes in
theorem conservative_step (s : State I) (enabled : Applicable learned s) :
    Applicable actual s ∧ apply learned s = apply actual s := by
  constructor
  · intro i v h
    exact enabled i v (pre_sound actual learned before valid common i v h)
  · funext i
    cases hl : learned.eff i with
    | some v =>
      have ha := effect_sound actual learned before after transition changes i v hl
      simp [apply,hl,ha]
    | none =>
      cases ha : actual.eff i with
      | none => simp [apply,hl,ha]
      | some v =>
        have hp := missing_effect_is_precondition actual learned before after transition common changes i v ha hl
        have hs := enabled i v hp
        simp [apply,hl,ha,hs]

inductive Executes (model : A → Action I) : State I → List A → State I → Prop where
  | nil (s : State I) : Executes model s [] s
  | cons {s t : State I} {a : A} {rest : List A} :
      Applicable (model a) s → Executes model (apply (model a) s) rest t →
      Executes model s (a :: rest) t

theorem plan_transfer (truth hypothesis : A → Action I)
    (safe : ∀ a s, Applicable (hypothesis a) s →
      Applicable (truth a) s ∧ apply (hypothesis a) s = apply (truth a) s)
    {s t : State I} {actions : List A} (h : Executes hypothesis s actions t) :
    Executes truth s actions t := by
  induction h with
  | nil s => exact Executes.nil s
  | @cons s t a rest enabled tail ih =>
    have hs := safe a s enabled
    apply Executes.cons hs.1
    rw [← hs.2]
    exact ih

#print axioms conservative_step
#print axioms plan_transfer

-- A rejected state satisfying every learned literal except i proves that i is
-- a true precondition, provided the true preconditions are contained in learned.
theorem boundary_failure (truth hypothesis : Action I) (s : State I) (i : I) (v : Bool)
    (sub : ∀ j w, truth.pre j = some w → hypothesis.pre j = some w)
    (present : hypothesis.pre i = some v)
    (other : ∀ j w, j ≠ i → hypothesis.pre j = some w → s j = w)
    (rejected : ¬ Applicable truth s) : truth.pre i = some v := by
  classical
  by_cases known : truth.pre i = some v
  · exact known
  · apply False.elim
    apply rejected
    intro j w actual
    by_cases eq : j = i
    · subst j
      have hyp := sub i w actual
      have value : w = v := Option.some.inj (hyp.symm.trans present)
      exact False.elim (known (value ▸ actual))
    · exact other j w eq (sub j w actual)

#print axioms boundary_failure
end SafeLearning
