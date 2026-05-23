with open("src/message/tests.rs", "r") as f:
    content = f.read()

content = content.replace("<<<<<<< HEAD\n\n=======\n>>>>>>> 3341496 (🧪 Add comprehensive tests for redact_secrets)", "")
content = content.replace("<<<<<<< HEAD\nfn redact_secrets_lowercase_indicators_without_matches_leave_output_unchanged() {\n=======\nfn redact_secrets_triggers_regex_if_lower_api_key_present() {\n>>>>>>> 3341496 (🧪 Add comprehensive tests for redact_secrets)", "fn redact_secrets_lowercase_indicators_without_matches_leave_output_unchanged() {")
content = content.replace("<<<<<<< HEAD\n    \n=======\n\n>>>>>>> 3341496 (🧪 Add comprehensive tests for redact_secrets)", "    ")

with open("src/message/tests.rs", "w") as f:
    f.write(content)

print("Conflict resolved.")
