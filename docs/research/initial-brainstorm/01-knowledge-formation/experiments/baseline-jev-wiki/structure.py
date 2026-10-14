"""Investigate zero-based array-reference mistakes without hiding the failed run."""
import run
from typesafe_sdk import Choice

run.CASES.clear()
blocks = {
    'title': 'Knowledge Garden Import Guide',
    'first_step': 'First, select the conversations you want to import.',
    'shell_command': 'python import_chats.py --dry-run',
    'explanation': 'The preview contains proposed changes and links to their source messages.',
    'caution': 'Warning: these changes are only proposals until reviewed.',
}
criteria = {'heading':'Section or document title.','instruction':'A step the reader should perform.','code':'A literal command or program fragment.','paragraph':'Explanatory prose.','warning':'A caution or warning.'}
expected = dict(zip(blocks, ['heading','instruction','code','paragraph','warning']))
for variant in ['named_paths', 'explicit_text']:
    run.add('structure_followup', variant, dict(blocks=blocks), {
        key: Choice(instructions=(f'What role does `blocks.{key}` play in this import guide?' if variant == 'named_paths' else {'question':'What role does this exact target block play in the import guide?', 'target_block':value}),criteria=criteria)
        for key,value in blocks.items()
    }, expected)

if __name__ == '__main__':
    run.main()
