#!/usr/bin/env python3
import os
import sys
import argparse
from pathlib import Path

def combine_paths(paths, output_file):
    """Combine multiple paths into a single text file."""
    with open(output_file, 'w', encoding='utf-8') as out:
        for path_str in paths:
            path = Path(path_str)
            
            if not path.exists():
                print(f"Warning: {path} does not exist, skipping...")
                continue
                
            out.write(f"\n{'='*80}\n")
            out.write(f"PATH: {path}\n")
            out.write(f"{'='*80}\n\n")
            
            if path.is_file():
                try:
                    with open(path, 'r', encoding='utf-8') as f:
                        out.write(f.read())
                        out.write('\n\n')
                except UnicodeDecodeError:
                    out.write(f"[Binary file - skipped]\n\n")
                except Exception as e:
                    out.write(f"[Error reading file: {e}]\n\n")
            
            elif path.is_dir():
                for root, dirs, files in os.walk(path):
                    # Skip common ignored directories
                    dirs[:] = [d for d in dirs if not d.startswith('.') and d not in ['node_modules', '__pycache__', 'target', 'dist', 'build']]
                    
                    for file in files:
                        if file.startswith('.'):
                            continue
                            
                        file_path = Path(root) / file
                        out.write(f"\n{'-'*60}\n")
                        out.write(f"FILE: {file_path}\n")
                        out.write(f"{'-'*60}\n\n")
                        
                        try:
                            with open(file_path, 'r', encoding='utf-8') as f:
                                out.write(f.read())
                                out.write('\n\n')
                        except UnicodeDecodeError:
                            out.write(f"[Binary file - skipped]\n\n")
                        except Exception as e:
                            out.write(f"[Error reading file: {e}]\n\n")

def main():
    parser = argparse.ArgumentParser(description='Combine multiple paths into a single text file')
    parser.add_argument('paths', nargs='+', help='Paths to combine')
    parser.add_argument('-o', '--output', help='Output file name (default: foldername.txt based on first path)')
    
    args = parser.parse_args()
    
    if args.output:
        output_file = args.output
    else:
        first_path = Path(args.paths[0])
        folder_name = first_path.name if first_path.is_dir() else first_path.stem
        output_file = f"{folder_name}.txt"
    
    combine_paths(args.paths, output_file)
    print(f"Combined {len(args.paths)} paths into {output_file}")

if __name__ == "__main__":
    main()