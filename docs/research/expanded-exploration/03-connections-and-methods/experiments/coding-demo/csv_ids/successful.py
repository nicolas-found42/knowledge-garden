def key(x): return x
assert key("007") != key("7"), "distinct customer IDs collapsed"
print("invariants passed")
