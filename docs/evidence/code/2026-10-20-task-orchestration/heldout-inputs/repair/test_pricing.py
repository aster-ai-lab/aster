import unittest
from pricing import total_price

class PricingTests(unittest.TestCase):
    def test_multiple(self):
        self.assertEqual(total_price([10, 20], 3), 33)
    def test_empty(self):
        self.assertEqual(total_price([], 7), 7)
    def test_negative(self):
        self.assertEqual(total_price([-4, 9], -2), 3)

if __name__ == "__main__":
    unittest.main()
