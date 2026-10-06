import Std

/- Selected observation obligations. Predicates model finite sets without
   imposing a particular data structure. These theorems do not verify Rust,
   native admission, or the existence of a finite interface for every program. -/
namespace SelectedProvenance

def image (tag : E → L → Prop) (support : E → Prop) : L → Prop :=
  fun l => ∃ e, support e ∧ tag e l

theorem image_read_union (tag : E → L → Prop) (reads : I → E → Prop) :
    image tag (fun e => ∃ i, reads i e) =
    (fun l => ∃ i, image tag (reads i) l) := by
  funext l; apply propext; constructor
  · rintro ⟨e, ⟨i, he⟩, hl⟩; exact ⟨i, e, he, hl⟩
  · rintro ⟨i, e, he, hl⟩; exact ⟨e, ⟨i, he⟩, hl⟩

theorem image_birth (tag : E → L → Prop) (support : E → Prop) (born : E) :
    image tag (fun e => e = born ∨ support e) =
    (fun l => tag born l ∨ image tag support l) := by
  funext l; apply propext; constructor
  · rintro ⟨e, h | h, hl⟩
    · subst e; exact Or.inl hl
    · exact Or.inr ⟨e, h, hl⟩
  · rintro (h | ⟨e, he, hl⟩)
    · exact ⟨born, Or.inl rfl, h⟩
    · exact ⟨e, Or.inr he, hl⟩

inductive Path (step : S → A → S → Prop) : S → List A → S → Prop
  | nil (s : S) : Path step s [] s
  | cons : step s a t → Path step t rest u → Path step s (a :: rest) u

theorem project_path (concrete : C → A → C → Prop) (abstract : Q → A → Q → Prop)
    (observe : C → Q)
    (forward : ∀ s a t, concrete s a t → abstract (observe s) a (observe t))
    (path : Path concrete s actions t) : Path abstract (observe s) actions (observe t) := by
  induction path with
  | nil s => exact .nil (observe s)
  | cons step _ ih => exact .cons (forward _ _ _ step) ih

-- The backward premise is at EVERY concrete representative, not a union of
-- outgoing transitions over the states that happen to share an observation.
theorem lift_path (concrete : C → A → C → Prop) (abstract : Q → A → Q → Prop)
    (observe : C → Q)
    (backward : ∀ s a q, abstract (observe s) a q → ∃ t, concrete s a t ∧ observe t = q)
    (path : Path abstract q actions r) :
    ∀ s, observe s = q → ∃ t, Path concrete s actions t ∧ observe t = r := by
  induction path with
  | nil q => intro s _; exact ⟨s, .nil s, by assumption⟩
  | cons edge _ ih =>
    intro s hs
    rw [← hs] at edge
    obtain ⟨middle, step, hm⟩ := backward s _ _ edge
    obtain ⟨t, tail, ht⟩ := ih middle hm
    exact ⟨t, .cons step tail, ht⟩

-- Nonempty-family requirement prevents vacuous necessity for unreachable actions.
def necessary (family : C → Prop) (has : C → L → Prop) (l : L) :=
  (∃ c, family c) ∧ ∀ c, family c → has c l
def minimal (family : C → Prop) (lt : C → C → Prop) (c : C) :=
  family c ∧ ¬ ∃ d, family d ∧ lt d c

theorem family_necessity (f g : C → Prop) (same : ∀ c, f c ↔ g c) (has : C → L → Prop) :
    necessary f has l ↔ necessary g has l := by
  constructor
  · rintro ⟨⟨c,hc⟩, h⟩; exact ⟨⟨c,(same c).mp hc⟩, fun d hd => h d ((same d).mpr hd)⟩
  · rintro ⟨⟨c,hc⟩, h⟩; exact ⟨⟨c,(same c).mpr hc⟩, fun d hd => h d ((same d).mp hd)⟩

theorem family_minimality (f g : C → Prop) (same : ∀ c, f c ↔ g c) (lt : C → C → Prop) :
    minimal f lt c ↔ minimal g lt c := by
  constructor
  · rintro ⟨hc,hn⟩; refine ⟨(same c).mp hc, ?_⟩
    rintro ⟨d,hd,hlt⟩; exact hn ⟨d,(same d).mpr hd,hlt⟩
  · rintro ⟨hc,hn⟩; refine ⟨(same c).mpr hc, ?_⟩
    rintro ⟨d,hd,hlt⟩; exact hn ⟨d,(same d).mp hd,hlt⟩

theorem recoverable_bits_injective (encode : (I → Bool) → Q) (recover : Q → I → Bool)
    (correct : ∀ h i, recover (encode h) i = h i) :
    ∀ h k, encode h = encode k → h = k := by
  intro h k he; funext i
  have h1 := correct h i
  rw [he] at h1
  exact h1.symm.trans (correct k i)

#print axioms image_read_union
#print axioms image_birth
#print axioms project_path
#print axioms lift_path
#print axioms family_necessity
#print axioms family_minimality
#print axioms recoverable_bits_injective
end SelectedProvenance
