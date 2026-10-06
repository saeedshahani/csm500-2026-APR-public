/- Abstract model of the native return/delay-return profile.
   The Rust readback supplies the actual installed body and captured identities.
   This file does not verify the Rust heap, matching or decoder implementation. -/
namespace Versioned
inductive Body where
  | ret | delay
  deriving DecidableEq, Repr
def cost : Body → Nat | .ret => 1 | .delay => 2
def evaluate (_ : Body) (payload : Nat) : Nat := payload
structure Version where
  identity : Nat
  body : Body
  deriving DecidableEq, Repr
structure Job where
  captured : Version
  payload : Nat
  deriving DecidableEq, Repr
structure State where
  installed : Version
  jobs : List Job
  deriving DecidableEq, Repr
structure Proposal where
  source : Version
  target : Version
  budget : Nat
  deriving DecidableEq, Repr
def publish (s : State) (p : Proposal) : Option State :=
  if p.source = s.installed ∧ p.source.identity ≠ p.target.identity ∧ cost p.target.body ≤ p.budget
  then some {s with installed := p.target}
  else none

theorem accepted_installation (h : publish s p = some t) :
    t.installed = p.target ∧ t.jobs = s.jobs ∧ cost t.installed.body ≤ p.budget := by
  unfold publish at h
  split at h
  · rename_i permitted
    cases Option.some.inj h
    exact ⟨rfl,rfl,permitted.2.2⟩
  · contradiction

theorem installed_result (h : publish s p = some t) (payload : Nat) :
    evaluate t.installed.body payload = payload ∧ cost t.installed.body ≤ p.budget :=
  ⟨rfl,(accepted_installation h).2.2⟩

def capture (s : State) (x : Nat) : Job := ⟨s.installed,x⟩
theorem old_jobs_preserved (h : publish s p = some t) : t.jobs = s.jobs :=
  (accepted_installation h).2.1
theorem new_job_captures (h : publish s p = some t) : (capture t x).captured = p.target :=
  (accepted_installation h).1

inductive Phase where
  | call | pending | done
  deriving DecidableEq, Repr
structure Task where
  job : Job
  phase : Phase
  deriving DecidableEq, Repr
def step (t : Task) : Option Task :=
  match t.phase, t.job.captured.body with
  | .call, .ret => some {t with phase := .done}
  | .call, .delay => some {t with phase := .pending}
  | .pending, .delay => some {t with phase := .done}
  | _, _ => none
theorem evaluation_preserves_capture (h : step t = some u) : u.job = t.job := by
  cases t with
  | mk j phase =>
    cases hbody : j.captured.body <;> cases phase <;> simp_all [step] <;> cases h <;> rfl

theorem return_finishes (j : Job) (h : j.captured.body = .ret) :
    step ⟨j,.call⟩ = some ⟨j,.done⟩ := by simp [step,h]
theorem delay_finishes (j : Job) (h : j.captured.body = .delay) :
    (step ⟨j,.call⟩).bind step = some ⟨j,.done⟩ := by simp [step,h]

-- These proposed post-states must fail correspondence, despite sharing an ID.
example : publish ⟨⟨0,.delay⟩,[]⟩ ⟨⟨0,.delay⟩,⟨1,.ret⟩,1⟩ ≠
    some ⟨⟨1,.delay⟩,[]⟩ := by decide
example : publish ⟨⟨0,.ret⟩,[]⟩ ⟨⟨0,.ret⟩,⟨1,.delay⟩,1⟩ = none := by decide
#print axioms accepted_installation
#print axioms installed_result
#print axioms evaluation_preserves_capture
#print axioms return_finishes
#print axioms delay_finishes
end Versioned
