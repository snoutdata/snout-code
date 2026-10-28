package demo

// Unrelated to Calc, so a change to Calc leaves it alone (and Zinc does not fall back to a full
// compile, which it does once half of the sources are invalidated).
object Alpha {
  val label: String = "Alpha"
}
