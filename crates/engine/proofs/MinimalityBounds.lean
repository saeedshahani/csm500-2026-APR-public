import Std

/- Conditional arithmetic only: graph reachability/capacity premises and the
   correspondence of these counts to the implementation remain manual. -/
namespace MinimalityBounds

theorem capacity (internal boundary edges degree : Nat)
    (connected : internal + boundary ≤ edges + 1)
    (bounded : edges ≤ degree * internal) :
    internal + boundary ≤ degree * internal + 1 := by omega

theorem unary_boundary (internal boundary edges : Nat)
    (connected : internal + boundary ≤ edges + 1)
    (bounded : edges ≤ internal) : boundary ≤ 1 := by omega

theorem binary_boundary (internal boundary edges : Nat)
    (connected : internal + boundary ≤ edges + 1)
    (bounded : edges ≤ 2 * internal) : boundary ≤ internal + 1 := by omega

theorem minimum_three_arcs (internal edges : Nat)
    (ordered_requires_auxiliary : 2 ≤ internal)
    (connected : internal + 2 ≤ edges + 1) : 3 ≤ edges := by omega

-- A one-root simple graph with two reachable sinks cannot distinguish them.
-- a,b denote its arcs to the two sinks; asymmetry requires a ≠ b.
theorem one_root_symmetry (a b : Bool) (left : a = true) (right : b = true) :
    ¬ a ≠ b := by simp [left, right]

#print axioms capacity
#print axioms unary_boundary
#print axioms binary_boundary
#print axioms minimum_three_arcs
#print axioms one_root_symmetry
end MinimalityBounds
