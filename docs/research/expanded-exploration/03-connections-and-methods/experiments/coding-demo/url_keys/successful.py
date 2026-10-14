from urllib.parse import urlsplit
def key(x):
 p=urlsplit(x); return (p.scheme.lower(),p.netloc.lower(),p.path,p.query)
assert key("https://example.invalid/a?view=all") != key("https://example.invalid/a?view=mine"), "query destinations collapsed"
print("invariants passed")
