def key(tenant,policy,text): return text.casefold()
assert key("A","v1","hello") != key("B","v1","HELLO"), "tenant isolation lost"
assert key("A","v1","hello") != key("A","v2","hello"), "policy revision ignored"
print("invariants passed")
