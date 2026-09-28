"""Safety checks for resuming the long compiler compatibility audit."""
import copy
import unittest
from check_compatibility import resume_results

class ResumeTests(unittest.TestCase):
    def setUp(self):
        self.current = dict(compiler_sha256='rust', elm_sha256='elm', planned_suites=2, input_sha256='inputs')
        self.commands = [('first', ['python', 'first.py']), ('second', ['python', 'second.py'])]
        self.old = {**self.current, 'results': [dict(suite='first', command=self.commands[0][1], passed=False, returncode=1)]}

    def test_retains_failed_prefix(self):
        self.assertEqual(resume_results(self.old, self.current, self.commands), self.old['results'])
        self.assertFalse(self.old['results'][0]['passed'])

    def test_rejects_changed_inputs_or_compilers(self):
        for key in self.current:
            with self.subTest(key=key):
                changed = {**self.current, key:'changed'}
                with self.assertRaises(ValueError):
                    resume_results(self.old, changed, self.commands)

    def test_rejects_reordered_changed_and_malformed_results(self):
        mutations = [('suite','second'), ('command',['python','changed.py']), ('passed','yes'), ('passed',True)]
        for key, value in mutations:
            with self.subTest(key=key,value=value):
                changed = copy.deepcopy(self.old)
                changed['results'][0][key] = value
                with self.assertRaises(ValueError):
                    resume_results(changed, self.current, self.commands)
        for invalid in [None, self.old['results']*3]:
            with self.assertRaises(ValueError):
                resume_results({**self.old,'results':invalid},self.current,self.commands)

    def test_accepts_empty_and_completed_prefix(self):
        self.assertEqual(resume_results({**self.current,'results':[]},self.current,self.commands),[])
        rows = [dict(suite=label,command=command,passed=True,returncode=0) for label,command in self.commands]
        self.assertEqual(resume_results({**self.current,'results':rows},self.current,self.commands),rows)

if __name__ == '__main__':
    unittest.main()
