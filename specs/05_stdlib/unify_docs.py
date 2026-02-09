import os
import re

def unify_file(filepath):
    with open(filepath, 'r') as f:
        content = f.read()

    # Pattern to find "What is this?" and "Why this exists" sections
    # It looks for:
    # 1. ## What ... (capture up to next ##)
    # 2. ## Why ... (capture up to next ##)
    
    # We want to remove the headers but keep the content, and place it right after the summary.
    # The summary is usually line 3 (after # Title \n \n Summary).
    
    lines = content.splitlines()
    new_lines = []
    
    what_content = []
    why_content = []
    
    msg_lines = [] # To store lines that are NOT what/why
    
    in_what = False
    in_why = False
    
    for line in lines:
        if line.startswith("## What"):
            in_what = True
            in_why = False
            continue
        if line.startswith("## Why"):
            in_what = False
            in_why = True
            continue
        if line.startswith("## ") and (in_what or in_why):
            in_what = False
            in_why = False
            
        if in_what:
            if line.strip() != "":
                what_content.append(line)
        elif in_why:
            if line.strip() != "":
                why_content.append(line)
        else:
            msg_lines.append(line)

    # Reconstruct
    # Find the end of the summary block (usually first h2 or code block)
    insert_idx = -1
    for i, line in enumerate(msg_lines):
        if line.startswith("## ") or line.startswith("```"):
            insert_idx = i
            break
            
    if insert_idx == -1:
        insert_idx = len(msg_lines)
        
    # Insert combined content
    combined = []
    if what_content:
        combined.extend(what_content)
        combined.append("")
    if why_content:
        combined.extend(why_content)
        combined.append("")

    final_lines = msg_lines[:insert_idx] + combined + msg_lines[insert_idx:]
    
    # Clean up multiple empty lines
    cleaned_lines = []
    for line in final_lines:
        if line.strip() == "" and (not cleaned_lines or cleaned_lines[-1].strip() == ""):
            continue
        cleaned_lines.append(line)

    return "\n".join(cleaned_lines)

def process_directory(root_dir):
    for dirpath, _, filenames in os.walk(root_dir):
        for filename in filenames:
            if filename.endswith(".md"):
                filepath = os.path.join(dirpath, filename)
                print(f"Processing {filepath}")
                try:
                    new_content = unify_file(filepath)
                    with open(filepath, 'w') as f:
                        f.write(new_content)
                except Exception as e:
                    print(f"Error processing {filepath}: {e}")

if __name__ == "__main__":
    process_directory(".")
