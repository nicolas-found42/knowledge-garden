"""Synthetic reserved-domain checks; destinations are never acquired."""
import unittest
from url_occurrences import normalize,occurrences
class URLs(unittest.TestCase):
 def test_equivalences(self):
  for a,b in [('HTTPS://Example.test:443/a','https://example.test/a'),('http://EXAMPLE.test:80/a','http://example.test/a')]:self.assertEqual(normalize(a)[0],normalize(b)[0])
 def test_distinctions(self):
  for a,b in [('https://example.test/A','https://example.test/a'),('https://example.test','https://example.test/'),('http://example.test/a','https://example.test/a'),('https://example.test/a?x=1','https://example.test/a?x=2'),('https://example.test/a?x=1&y=2','https://example.test/a?y=2&x=1'),('https://example.test/a#x','https://example.test/a#y'),('https://sub.example.test/a','https://example.test/a'),('https://example.test/a?utm_source=x','https://example.test/a'),('https://example.test/%2Fa','https://example.test//a'),('https://example.test/a:443','https://example.test/a'),('https://example.test:444/a','https://example.test/a')]:self.assertNotEqual(normalize(a)[0],normalize(b)[0])
 def test_occurrences(self):
  s='[read](https://example.test/a) then https://example.test/a and https://example.test/wiki_(x).'
  os=list(occurrences({'message':s}));self.assertEqual(len(os),3)
  for o in os:self.assertEqual(s[o['start']:o['end']],o['original']);self.assertFalse(o['destination_fetched']);self.assertEqual(o['status'],'merely_linked')
 def test_collision_is_lossless(self):
  os=list(occurrences(['HTTPS://EXAMPLE.test:443/a','https://example.test/a']))
  self.assertEqual(os[0]['identity'],os[1]['identity']);self.assertNotEqual(os[0]['original'],os[1]['original'])
 def test_invalid_port_retained(self):
  o=list(occurrences('https://example.test:bad/a'))[0];self.assertIsNone(o['identity']);self.assertEqual(o['original'],'https://example.test:bad/a')
 def test_empty_delimiters_and_port_zero(self):
  for a,b in [('https://example.test/a?','https://example.test/a'),('https://example.test/a#','https://example.test/a'),('https://example.test:0/a','https://example.test/a')]:self.assertNotEqual(normalize(a)[0],normalize(b)[0])
 def test_punctuation_recovery(self):
  o=list(occurrences('https://example.test/path!'))[0]
  self.assertFalse(o['delimiter_trimmed']);self.assertEqual(o['original'],'https://example.test/path!');self.assertEqual(o['lexical_match'],'https://example.test/path!')
  self.assertNotEqual(normalize(o['original'])[0],normalize('https://example.test/path')[0])
if __name__=='__main__':unittest.main()
