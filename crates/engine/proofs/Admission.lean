import Std

/- Owned-field ground transactions and a finite reference search.
   No quotation, rank decoding, Rust, or compiler verification is assumed here. -/
namespace Admission

abbrev Atom := Nat × List Nat
abbrev Graph := List Atom
def Live (g : Graph) (i : Nat) : Prop := i ∈ g.map Prod.fst
def Closed (g : Graph) : Prop := ∀ a ∈ g, ∀ t ∈ a.2, Live g t
def Fresh (g f : Graph) : Prop := ∀ a ∈ f, ¬ Live g a.1
def Dangling (g : Graph) (d : List Nat) : Prop :=
  ∀ a ∈ g, a.1 ∉ d → ∀ t ∈ a.2, t ∉ d
def patch (g : Graph) (d : List Nat) (f : Graph) : Graph :=
  g.filter (fun a => !(d.contains a.1)) ++ f
def FreshClosed (g : Graph) (d : List Nat) (f : Graph) : Prop :=
  ∀ a ∈ f, ∀ t ∈ a.2, (Live g t ∧ t ∉ d) ∨ Live f t

theorem mem_patch (a : Atom) (g : Graph) (d : List Nat) (f : Graph) :
    a ∈ patch g d f ↔ (a ∈ g ∧ a.1 ∉ d) ∨ a ∈ f := by
  simp [patch]

theorem live_patch (g : Graph) (d : List Nat) (f : Graph) (i : Nat) :
    Live (patch g d f) i ↔ (Live g i ∧ i ∉ d) ∨ Live f i := by
  simp only [Live, List.mem_map, mem_patch]
  constructor
  · rintro ⟨a, (⟨ha, hd⟩ | hf), hi⟩
    · exact Or.inl ⟨⟨a, ha, hi⟩, hi ▸ hd⟩
    · exact Or.inr ⟨a, hf, hi⟩
  · rintro (⟨⟨a, ha, hi⟩, hd⟩ | ⟨a, hf, hi⟩)
    · exact ⟨a, Or.inl ⟨ha, hi ▸ hd⟩, hi⟩
    · exact ⟨a, Or.inr hf, hi⟩

theorem preservation (g : Graph) (d : List Nat) (f : Graph)
    (old : Closed g) (dangling : Dangling g d) (added : FreshClosed g d f) :
    Closed (patch g d f) := by
  intro a ha t ht
  apply (live_patch g d f t).mpr
  rcases (mem_patch a g d f).mp ha with ⟨hg, hd⟩ | hf
  · exact Or.inl ⟨old a hg t ht, dangling a hg hd t ht⟩
  · exact added a hf t ht

theorem exact_admission (g : Graph) (d : List Nat) (f : Graph)
    (old : Closed g) (fresh : Fresh g f) :
    Closed (patch g d f) ↔ Dangling g d ∧ FreshClosed g d f := by
  constructor
  · intro post
    constructor
    · intro a ha hd t ht htd
      have h := (live_patch g d f t).mp (post a ((mem_patch a g d f).mpr (Or.inl ⟨ha,hd⟩)) t ht)
      rcases h with ⟨_, hn⟩ | hf
      · exact hn htd
      · rcases List.mem_map.mp hf with ⟨b, hb, he⟩
        exact fresh b hb (he ▸ old a ha t ht)
    · intro a ha t ht
      exact (live_patch g d f t).mp (post a ((mem_patch a g d f).mpr (Or.inr ha)) t ht)
  · rintro ⟨hd, hf⟩
    exact preservation g d f old hd hf

def closedCheck (g : Graph) : Bool :=
  g.all fun a => a.2.all fun t => (g.map Prod.fst).contains t
def freshCheck (g f : Graph) : Bool :=
  f.all fun a => !((g.map Prod.fst).contains a.1)
def admissionCheck (g : Graph) (d : List Nat) (f : Graph) : Bool :=
  (g.all fun a => d.contains a.1 || a.2.all fun t => !(d.contains t)) &&
  (f.all fun a => a.2.all fun t =>
    (((g.map Prod.fst).contains t) && !(d.contains t)) || (f.map Prod.fst).contains t)

theorem closedCheck_exact (g : Graph) : closedCheck g = true ↔ Closed g := by
  simp [closedCheck, Closed, Live]
theorem freshCheck_exact (g f : Graph) : freshCheck g f = true ↔ Fresh g f := by
  simp [freshCheck, Fresh, Live]
theorem checker_predicates (g : Graph) (d : List Nat) (f : Graph) :
    admissionCheck g d f = true ↔ Dangling g d ∧ FreshClosed g d f := by
  simp [admissionCheck, Dangling, FreshClosed, Live]
  intro _
  constructor
  · intro h a b hab hn
    exact (h a b hab).resolve_left hn
  · intro h a b hab
    by_cases hd : a ∈ d
    · exact Or.inl hd
    · exact Or.inr (h a b hab hd)
theorem checker_exact (g : Graph) (d : List Nat) (f : Graph)
    (old : Closed g) (fresh : Fresh g f) :
    admissionCheck g d f = true ↔ Closed (patch g d f) := by
  exact (checker_predicates g d f).trans (exact_admission g d f old fresh).symm

/- Preserve the complete modelled pair (identity and ordered fields).
   This model does not contain native kinds, labels or other attributes. -/
theorem retained_immutable (g : Graph) (d : List Nat) (f : Graph)
    (a : Atom) (old : a ∈ g) (kept : a.1 ∉ d) : a ∈ patch g d f := by
  exact (mem_patch a g d f).mpr (Or.inl ⟨old,kept⟩)

def Unique (g : Graph) : Prop := (g.map Prod.fst).Nodup
theorem unique_preserved (g : Graph) (d : List Nat) (f : Graph)
    (old : Unique g) (added : Unique f) (fresh : Fresh g f) :
    Unique (patch g d f) := by
  unfold Unique patch
  rw [List.map_append]
  change List.Pairwise (· ≠ ·) _
  rw [List.pairwise_append]
  refine ⟨?_, added, ?_⟩
  · exact old.sublist (List.filter_sublist.map _)
  · intro i hi j hj eq
    rcases List.mem_map.mp hi with ⟨a, ha, he⟩
    rcases List.mem_map.mp hj with ⟨b, hb, hf⟩
    have hag : a ∈ g := (List.mem_filter.mp ha).1
    apply fresh b hb
    exact List.mem_map.mpr ⟨a, hag, he.trans (eq.trans hf.symm)⟩

/- Lists enumerate all total assignments, including noninjective proposals.
   The exact complete-match predicate is supplied separately, never inferred
   from domain size. Filtering can express injectivity and field constraints. -/
def assignments (host : List Nat) : Nat → List (List Nat)
  | 0 => [[]]
  | n+1 => host.flatMap fun x => (assignments host n).map (x :: ·)

theorem assignments_exact (host xs : List Nat) (n : Nat) :
    xs ∈ assignments host n ↔ xs.length = n ∧ ∀ x ∈ xs, x ∈ host := by
  induction n generalizing xs with
  | zero => cases xs <;> simp [assignments]
  | succ n ih =>
    cases xs with
    | nil => simp [assignments]
    | cons x xs => simp [assignments, ih, and_assoc, and_left_comm, and_comm]

def search (host : List Nat) (n : Nat) (accept : List Nat → Bool) : List (List Nat) :=
  (assignments host n).filter accept

theorem search_exact (host xs : List Nat) (n : Nat) (accept : List Nat → Bool) :
    xs ∈ search host n accept ↔
      xs.length = n ∧ (∀ x ∈ xs, x ∈ host) ∧ accept xs = true := by
  simp [search, assignments_exact, and_assoc]

/- Necessary hypotheses are tested by kernel-reduced small countermodels. -/
example : closedCheck [(0,[1]),(1,[])] = true := by decide
example : closedCheck (patch [(0,[1]),(1,[])] [1] []) = false := by decide
example : closedCheck (patch [(0,[1]),(1,[])] [1] [(1,[])]) = true := by decide
example : freshCheck [(0,[1]),(1,[])] [(1,[])] = false := by decide
example : admissionCheck [(0,[])] [] [(1,[2])] = false := by decide
example : admissionCheck [] [] [(0,[1]),(1,[0])] = true := by decide

#print axioms preservation
#print axioms exact_admission
#print axioms checker_exact
#print axioms retained_immutable
#print axioms unique_preserved
#print axioms assignments_exact
#print axioms search_exact
end Admission
