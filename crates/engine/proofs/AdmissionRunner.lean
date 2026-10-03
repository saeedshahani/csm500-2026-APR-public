import Admission
import Lean

open Lean Admission

def graphOf (value : Json) : Except String Graph := do
  (← value.getArr?).toList.mapM fun row => do
    let row ← row.getArr?
    if row.size != 2 then throw "atom must be [id, fields]"
    let id ← row[0]!.getNat?
    let fields ← row[1]!.getArr?
    return (id, ← fields.toList.mapM Json.getNat?)

def answer (value : Json) : Except String Json := do
  let g ← graphOf (← value.getObjVal? "graph")
  let f ← graphOf (← value.getObjVal? "fresh")
  let d ← (← (← value.getObjVal? "delete").getArr?).toList.mapM Json.getNat?
  return Json.mkObj [
    ("closed", toJson (closedCheck g)),
    ("fresh", toJson (freshCheck g f)),
    ("accepted", toJson (admissionCheck g d f)),
    ("post_closed", toJson (closedCheck (patch g d f))),
    ("post", toJson (patch g d f))]

def main : IO Unit := do
  let input ← IO.getStdin
  let output ← IO.getStdout
  repeat
    let line ← input.getLine
    if line.isEmpty then break
    match Json.parse line >>= answer with
    | .error e => throw (IO.userError e)
    | .ok result => output.putStrLn result.compress
