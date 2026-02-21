import os
import re
import json
import subprocess

def slugify(s):
    # Remove markdown links
    s = re.sub(r'\[([^\]]+)\]\([^)]+\)', r'\1', s)
    # Remove inline code marks
    s = s.replace('`', '')
    # Strip leading numbering like "8.1. " or "8 " or "8.1.1" or "8. "
    s = re.sub(r'^(\d+\.)*\d+\.?\s+', '', s)
    s = s.lower()
    s = re.sub(r'[^a-z0-9]+', '_', s)
    return s.strip('_')

repo_root = '.'
manifest_path = 'specs/snippets/manifest.json'
with open(manifest_path, 'r') as f:
    manifest = json.load(f)

md_files = []
for root, _, files in os.walk('specs'):
    for file in files:
        if file.endswith('.md') and 'node_modules' not in root and '.vitepress' not in root:
            md_files.append(os.path.join(root, file))

renames = {} # old repo-relative path -> new repo-relative path

for md_file in md_files:
    try:
        with open(md_file, 'r') as f:
            lines = f.readlines()
    except Exception:
        continue
    
    current_heading = "intro"
    heading_counts = {}
    
    # Pass 1: count blocks per heading
    for line in lines:
        m = re.match(r'^#+\s+(.*)', line)
        if m:
            s_val = slugify(m.group(1))
            if s_val:
                current_heading = s_val
            else:
                current_heading = "section"
        
        if '<<<' in line and '{aivi}' in line and '.aivi' in line:
            heading_counts[current_heading] = heading_counts.get(current_heading, 0) + 1

    # Pass 2: generate renames and update markdown
    current_heading = "intro"
    heading_seen = {}
    
    new_lines = []
    changed = False
    
    for line in lines:
        m = re.match(r'^#+\s+(.*)', line)
        if m:
            s_val = slugify(m.group(1))
            if s_val:
                current_heading = s_val
            else:
                current_heading = "section"
                
        inc_match = re.search(r'<<<\s+([^\{]+\.aivi)', line)
        if inc_match:
            rel_path = inc_match.group(1).strip()
            if 'block_' in os.path.basename(rel_path):
                md_dir = os.path.dirname(md_file)
                abs_snippet_path = os.path.normpath(os.path.join(md_dir, rel_path))
                repo_rel_snippet_path = os.path.relpath(abs_snippet_path, repo_root)
                
                if os.path.exists(repo_rel_snippet_path):
                    idx = heading_seen.get(current_heading, 0) + 1
                    heading_seen[current_heading] = idx
                    
                    total = heading_counts.get(current_heading, 1)
                    
                    suffix = f"_{idx:02d}" if total > 1 else ""
                    new_basename = f"{current_heading}{suffix}.aivi"
                    
                    new_repo_rel = os.path.join(os.path.dirname(repo_rel_snippet_path), new_basename)
                    renames[repo_rel_snippet_path] = new_repo_rel
                    
                    new_rel_path = os.path.relpath(os.path.join(repo_root, new_repo_rel), md_dir)
                    if not new_rel_path.startswith('.'):
                        new_rel_path = './' + new_rel_path
                    new_rel_path = new_rel_path.replace(os.sep, '/')
                    
                    line = line.replace(rel_path, new_rel_path)
                    changed = True
            
        new_lines.append(line)
        
    if changed:
        with open(md_file, 'w') as f:
            f.writelines(new_lines)

# update manifest
for entry in manifest.get('snippets', []):
    old_path = entry['path']
    normalized_old_path = os.path.normpath(old_path)
    if normalized_old_path in renames:
        new_path = renames[normalized_old_path]
        entry['path'] = new_path.replace(os.sep, '/')
        
        old_basename = os.path.splitext(os.path.basename(old_path))[0]
        new_basename = os.path.splitext(os.path.basename(new_path))[0]
        
        if old_basename in entry['module']:
            entry['module'] = entry['module'].replace(old_basename, new_basename)
        else:
            short_new_base = new_basename.replace('-', '_').replace(' ', '_')
            entry['module'] = f"{entry['module']}_{short_new_base}"

with open(manifest_path, 'w') as f:
    json.dump(manifest, f, indent=2)
    f.write('\n')

for old_path, new_path in renames.items():
    if os.path.exists(old_path):
        subprocess.run(['git', 'mv', old_path, new_path])
