from collections import Counter
import json


def get_name(log_entry):
    action = log_entry['action']
    if "Publish" in action:
        return action["Publish"]["package"]["id"]
    else:
        return action["Download"]["package"]["id"]


# find most popular downloads
log_entries = map(json.loads, open("pypi-output.json"))
counts = Counter(map(get_name, log_entries))

# make initialize of top 30 downloads
top_30 = [c[0] for c in counts.most_common(100)]
new_initial = [json.dumps(c) + "\n" for c in map(json.loads, open("pypi-initial.json")) if c['action']["Publish"]["package"]["id"] in top_30]
with open("small-pypi-initial.json", "w") as f:
    f.writelines(new_initial)

# loop through events
log_entries = map(json.loads, open("pypi-output.json"))

new_entries = [json.dumps(i) + "\n" for i in log_entries if get_name(i) in top_30][:10000]
with open("small-pypi-output.json", 'w') as f:
    f.writelines(new_entries)
