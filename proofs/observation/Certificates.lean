-- Membership evidence for existential path observations. Logical obligations only;
-- regular-language closure and finite witness selection are supplied in the written argument.
import Std
namespace ObservationFrontier
variable {Word Query : Type}
def Answer (P : Query → Word → Prop) (L : Word → Prop) (j : Query) : Prop :=
  ∃ w, P j w ∧ L w
def Consistent (E H U : Word → Prop) : Prop := ∀ w, E w → (H w ↔ U w)
def Certifies (P : Query → Word → Prop) (E H : Word → Prop) : Prop :=
  ∀ U, Consistent E H U → ∀ j, Answer P H j ↔ Answer P U j

theorem sufficient (P : Query → Word → Prop) (E H : Word → Prop)
    (positive : ∀ j, Answer P H j → ∃ w, E w ∧ P j w ∧ H w)
    (negative : ∀ j, ¬ Answer P H j → ∀ w, P j w → E w) :
    Certifies P E H := by
  intro U agree j
  constructor
  · intro h
    obtain ⟨w, he, hp, hh⟩ := positive j h
    exact ⟨w, hp, (agree w he).mp hh⟩
  · intro h
    classical
    apply Classical.byContradiction
    intro no
    obtain ⟨w, hp, hu⟩ := h
    exact no ⟨w, hp, (agree w (negative j no w hp)).mpr hu⟩

theorem positive_necessary (P : Query → Word → Prop) (E H : Word → Prop)
    (cert : Certifies P E H) (j : Query) (yes : Answer P H j) :
    ∃ w, E w ∧ P j w ∧ H w := by
  classical
  apply Classical.byContradiction
  intro no
  let U := fun w => H w ∧ ¬ P j w
  have agree : Consistent E H U := by
    intro w he
    constructor
    · intro hh
      exact ⟨hh, fun hp => no ⟨w, he, hp, hh⟩⟩
    · exact fun hu => hu.1
  obtain ⟨w, hp, hu⟩ := (cert U agree j).mp yes
  exact hu.2 hp

theorem negative_necessary (P : Query → Word → Prop) (E H : Word → Prop)
    (cert : Certifies P E H) (j : Query) (no : ¬ Answer P H j)
    (z : Word) (path : P j z) : E z := by
  classical
  apply Classical.byContradiction
  intro missing
  let U := fun w => H w ∨ w = z
  have agree : Consistent E H U := by
    intro w he
    constructor
    · exact Or.inl
    · intro hu
      rcases hu with hh | hz
      · exact hh
      · subst w
        exact False.elim (missing he)
  exact no ((cert U agree j).mpr ⟨z,path,Or.inr rfl⟩)

theorem characterization (P : Query → Word → Prop) (E H : Word → Prop) :
    Certifies P E H ↔
    (∀ j, Answer P H j → ∃ w, E w ∧ P j w ∧ H w) ∧
    (∀ j, ¬ Answer P H j → ∀ w, P j w → E w) := by
  constructor
  · intro h
    exact ⟨positive_necessary P E H h, negative_necessary P E H h⟩
  · rintro ⟨pos,neg⟩
    exact sufficient P E H pos neg

-- Core compatibility obligation behind the repaired quantitative reduction.
-- Removing candidate words is insufficient if an added word restores a query.
def Perturb (P : Query → Word → Prop) (H A : Word → Prop) (J : Query → Prop)
    (w : Word) : Prop := (H w ∧ ∀ j, J j → ¬ P j w) ∨ A w

theorem compatible_removal (P : Query → Word → Prop) (H A : Word → Prop)
    (J : Query → Prop) (compatible : ∀ w, A w → ∀ j, J j → ¬ P j w)
    (j : Query) (selected : J j) : ¬ Answer P (Perturb P H A J) j := by
  rintro ⟨w,hp,hu⟩
  rcases hu with kept | added
  · exact kept.2 j selected hp
  · exact compatible w added j selected hp

theorem added_witness (P : Query → Word → Prop) (H A : Word → Prop)
    (J : Query → Prop) (j : Query) (w : Word) (added : A w) (path : P j w) :
    Answer P (Perturb P H A J) j := ⟨w,path,Or.inr added⟩

-- All observed positives and exhaustively observed negatives remain fixed.
theorem protected_answer (P : Query → Word → Prop) (E H U : Word → Prop)
    (agree : Consistent E H U) (j : Query)
    (guard : (∃ w, E w ∧ P j w ∧ H w) ∨
                 ((¬ Answer P H j) ∧ ∀ w, P j w → E w)) :
    Answer P H j ↔ Answer P U j := by
  rcases guard with pos | neg
  · obtain ⟨w,he,hp,hh⟩ := pos
    exact ⟨fun _ => ⟨w,hp,(agree w he).mp hh⟩, fun _ => ⟨w,hp,hh⟩⟩
  · constructor
    · exact fun h => False.elim (neg.1 h)
    · rintro ⟨w,hp,hu⟩
      exact ⟨w,hp,(agree w (neg.2 w hp)).mpr hu⟩
#print axioms characterization
#print axioms compatible_removal
#print axioms protected_answer

-- Any alternative U admits an observation-equivalent regular normal form when
-- H and all P_j are regular and Query is finite. Regularity and finite witness
-- selection are supplied by the paper; these are the load-bearing logical parts.
def Regularize (P : Query → Word → Prop) (H U : Word → Prop)
    (witnesses : List Word) (w : Word) : Prop :=
  (H w ∧ ∀ j, (¬ Answer P U j) → ¬ P j w) ∨ w ∈ witnesses

theorem regularize_answers (P : Query → Word → Prop) (H U : Word → Prop)
    (witnesses : List Word)
    (sound : ∀ w, w ∈ witnesses → U w)
    (covers : ∀ j, Answer P U j → ∃ w, w ∈ witnesses ∧ P j w)
    (j : Query) : Answer P (Regularize P H U witnesses) j ↔ Answer P U j := by
  constructor
  · rintro ⟨w,hp,hv⟩
    rcases hv with kept | added
    · apply Classical.byContradiction
      intro no
      exact kept.2 j no hp
    · exact ⟨w,hp,sound w added⟩
  · intro hu
    obtain ⟨w,hw,hp⟩ := covers j hu
    exact ⟨w,hp,Or.inr hw⟩

theorem regularize_disagreement (P : Query → Word → Prop) (H U : Word → Prop)
    (witnesses : List Word) (sound : ∀ w, w ∈ witnesses → U w) (w : Word) :
    ((H w ∧ ¬ Regularize P H U witnesses w) ∨
     (¬ H w ∧ Regularize P H U witnesses w)) →
    ((H w ∧ ¬ U w) ∨ (¬ H w ∧ U w)) := by
  intro diff
  rcases diff with removed | added
  · apply Or.inl
    refine ⟨removed.1, ?_⟩
    intro hu
    apply removed.2
    apply Or.inl
    refine ⟨removed.1, ?_⟩
    intro j no hp
    exact no ⟨w,hp,hu⟩
  · apply Or.inr
    refine ⟨added.1, ?_⟩
    rcases added.2 with kept | selected
    · exact False.elim (added.1 kept.1)
    · exact sound w selected
#print axioms regularize_answers
#print axioms regularize_disagreement
end ObservationFrontier
