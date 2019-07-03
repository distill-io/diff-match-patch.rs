use std::fmt;
use core::char;
use std::iter::FromIterator;
use std::collections::HashMap;
extern crate  url;

use url::percent_encoding::{
    utf8_percent_encode,
    percent_decode,
    DEFAULT_ENCODE_SET,
    USERINFO_ENCODE_SET,
    };
#[allow(dead_code)]
pub struct Dmp {
    pub text1: String,
    pub text2: String,
    pub edit_cost: i32,
    pub match_distance: i32,
    pub patch_margin: i32,
    pub match_maxbits: i32,
    pub match_threshold: f32,
    pub patch_delete_threshold: f32
}

pub struct Diff {
    pub operation: i32,
    pub text: String,
}
pub struct Patch {
    pub diffs: Vec<Diff>,
    pub start1: i32,
    pub start2: i32,
    pub length1: i32,
    pub length2: i32
}
impl Diff {
    #[allow(dead_code)]
    pub fn new(operation: i32, text: String) -> Diff {
        Diff {
            operation: operation,
            text: text
        }
    }
}

impl PartialEq for Diff {
    fn eq(&self, other: &Self) -> bool {
        ((self.operation == other.operation) & (self.text == other.text))
    }
}

impl PartialEq for Patch {
    fn eq(&self, other: &Self) -> bool {
        ((self.diffs == other.diffs) & 
        (self.start1 == other.start1) & 
        (self.start2 == other.start2) &
        (self.length1 == other.length1) &
        (self.length2 == other.length2))
    }
}

impl Patch {
    #[allow(dead_code)]
    pub fn new(diffs: Vec<Diff>, start1: i32, start2: i32, length1: i32, length2: i32) -> Patch {
        Patch {
            diffs: diffs,
            start1: start1,
            start2: start2,
            length1: length1,
            length2: length2
        }
    } 
}
#[allow(dead_code)]
fn min(x: i32, y: i32) -> i32 {
    if x>y {
        return y;
    }
    x
}

#[allow(dead_code)]
fn min1(x: f32, y: f32) -> f32 {
    if x > y {
        return y;
    }
    return x;
}

#[allow(dead_code)]
fn max(x: i32, y: i32) -> i32 {
    if x>y {
        return x;
    }
    y
}

#[allow(dead_code)]
fn find_char(cha: char, text: &Vec<char>, start: usize) -> i32 {
    for i in start..text.len() {
        if text[i] == cha {
            return i as i32;
        }
    }
    return -1;
}
impl fmt::Debug for Diff {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "\n  {{ {}: {} }}", self.operation, self.text)
    }
}

impl fmt::Debug for Patch {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{diffs:\n {:?},\n start1: {},\n start2: {},\n length1: {},\n length2: {} }}", self.diffs, self.start1, self.start2, self.length1, self.length2)
    }
}









impl Dmp {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Dmp { patch_delete_threshold: 0.5, text1: "".to_string(), text2: "".to_string(), edit_cost: 0, match_distance: 1000, patch_margin: 4, match_maxbits: 32, match_threshold: 0.5}
    }

    #[allow(dead_code)]
    pub fn diff_main(&mut self, text1: &str, text2: &str, checklines: bool) -> Vec<Diff> {
        if text1.is_empty() && text2.is_empty() {
            return vec![];
        }
        else if text1.is_empty() {
            return vec![Diff::new(1, text2.to_string())];
        }
        else if text2.is_empty() {
            return vec![Diff::new(-1, text1.to_string())];
        }
        if text1 == text2 {
            return vec![Diff::new(0, text1.to_string())];
        }
        let mut char1: Vec<char> = text1.chars().collect();
        let mut char2: Vec<char> = text2.chars().collect();
        let mut commonlength = self.diff_common_prefix(&char1, &char2) as usize;
        let commonprefix = Vec::from_iter(char1[0..commonlength].iter().cloned());
        char1 = Vec::from_iter(char1[commonlength..].iter().cloned());
        char2 = Vec::from_iter(char2[commonlength..].iter().cloned());
        commonlength = self.diff_common_suffix(&char1, &char2) as usize;
        let commonsuffix = Vec::from_iter(char1[(char1.len() - commonlength)..char1.len()].iter().cloned());
        char1 = Vec::from_iter(char1[..(char1.len() - commonlength)].iter().cloned());
        char2 = Vec::from_iter(char2[..(char2.len() - commonlength)].iter().cloned());
        let mut diffs: Vec<Diff> = Vec::new();
        if commonprefix.is_empty() == false {
            diffs.push(Diff::new(0, commonprefix.iter().collect()));
        }
        let temp = self.diff_compute(&char1, &char2, checklines);
        for z in temp {
            diffs.push(z);
        }
        if commonsuffix.is_empty() == false {
            diffs.push(Diff::new(0, commonsuffix.iter().collect()));
        }
        self.diff_cleanup_merge(&mut diffs);
        return diffs;
    }

    #[allow(dead_code)]
    fn diff_compute(&mut self, text1: &Vec<char>, text2: &Vec<char>, checklines: bool) -> Vec<Diff> {
        let mut diffs: Vec<Diff> = Vec::new();
        if text1.is_empty() {
            diffs.push(Diff::new(1, text2.iter().collect()));
            return diffs;
        }
        else if text2.is_empty() {
            diffs.push(Diff::new(-1, text1.iter().collect()));
            return diffs;
        }
        {
            let len1 = text1.len();
            let len2 = text2.len();
            let longtext;
            let shorttext;
            if len1 >= len2 {
                longtext = text1;
                shorttext = text2;
            }
            else {
                longtext = text2;
                shorttext = text1;
            }
            let i = self.kmp(longtext, shorttext, 0);
            if i != -1 {
                if len1 > len2 {
                    if i != 0 {
                        diffs.push(Diff::new(-1, (text1[0..(i as usize)]).iter().collect()));
                    }
                    diffs.push(Diff::new(0, text2.iter().collect()));
                    if i as usize + text2.len() != text1.len() {
                        diffs.push(Diff::new(-1, text1[((i as usize) + text2.len())..].iter().collect()));
                    }
                }
                else {
                    if i != 0 {
                        diffs.push(Diff::new(1, (text2[0..(i as usize)]).iter().collect()));
                    }
                    diffs.push(Diff::new(0, text1.iter().collect()));
                    if (i as usize) + text1.len() != text2.len() {
                        diffs.push(Diff::new(1, text2[((i as usize) + text1.len())..].iter().collect()));
                    }
                }
                return diffs;

            }
            if shorttext.len() == 1 {
                diffs.push(Diff::new(-1, text1.iter().collect()));
                diffs.push(Diff::new(1, text2.iter().collect()));
                return diffs;
            }
        }
        let hm = self.diff_half_match(text1, text2);
        if hm.len() > 0 {
            let text1_a = hm[0].clone();
            let text1_b = hm[1].clone();
            let text2_a = hm[2].clone();
            let text2_b = hm[3].clone();
            let mid_common = hm[4].clone();
            let mut diffs_a = self.diff_main(text1_a.as_str(), text2_a.as_str(), checklines);
            let diffs_b = self.diff_main(text1_b.as_str(), text2_b.as_str(), checklines);
            diffs_a.push(Diff::new(0, mid_common));
            for x in diffs_b {
                diffs_a.push(x);
            }
            return diffs_a;
        }
        if checklines && text1.len() > 100 && text2.len() > 100 {
            return self.diff_linemode(text1, text2);
        }
        return self.diff_bisect(text1, text2);
    }
    
    fn kmp(&mut self, text1: &[char], text2: &[char], ind: usize) -> i32 {
        if text2.is_empty() {
            return ind as i32;
        }
        if text1.is_empty() {
            return -1;
        }
        let len1 = text1.len();
        let len2 = text2.len();
        let mut patern: Vec<usize> = Vec::new();
        patern.push(0);
        let mut len = 0;
        let mut i = 1;
        while i < len2 {
            if text2[i] == text2[len] {
                len += 1;
                patern.push(len);
                i += 1;
            }
            else {
                if len == 0 {
                    patern.push(0);
                    i += 1;
                }
                else {
                    len = patern[len - 1];
                }
            }
        }
        i = ind;
        len = 0;
        while i < len1 {
            if text1[i] == text2[len] {
                len += 1;
                i += 1;
                if len == len2 {
                    return (i - len) as i32;
                }
            }
            else {
                if len == 0 {
                    i += 1;
                }
                else {
                    len = patern[len - 1];
                }
            }
        }
        -1
    }
    
    #[allow(dead_code)]
    fn rkmp(&mut self, text1: &[char], text2: &[char], ind: usize) -> i32 {
        if text2.is_empty() {
            return ind as i32;
        }
        if text1.is_empty() {
            return -1;
        }
        let len2 = text2.len();
        let mut patern: Vec<usize> = Vec::new();
        patern.push(0);
        let mut len = 0;
        let mut i = 1;
        while i < len2 {
            if text2[i] == text2[len] {
                len += 1;
                patern.push(len);
                i += 1;
            }
            else {
                if len == 0 {
                    patern.push(0);
                    i += 1;
                }
                else {
                    len = patern[len - 1];
                }
            }
        }
        i = 0;
        len = 0;
        let mut ans: i32 = -1;
        while i <= ind {
            if text1[i] == text2[len] {
                len += 1;
                i += 1;
                if len == len2 {
                    ans = (i - len) as i32;
                    len = patern[len-1];
                }
            }
            else {
                if len == 0 {
                    i += 1;
                }
                else {
                    len = patern[len - 1];
                }
            }
        }
        ans
    }

    pub fn diff_linemode(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> Vec<Diff> {
        let (text3, text4, linearray) = self.diff_lines_tochars(text1, text2);
        let mut dmp = Dmp::new();
        let  mut diffs: Vec<Diff> = dmp.diff_main(text3.as_str(), text4.as_str(), false);
        self.diff_chars_tolines(&mut diffs, &linearray);
        self.diff_cleanup_semantic(&mut diffs);
        diffs.push(Diff::new(0,"".to_string()));
        let mut count_delete = 0;
        let mut count_insert = 0;
        let mut text_delete: String = "".to_string();
        let mut text_insert: String = "".to_string();
        let mut pointer = 0;
        let mut temp: Vec<Diff> = vec![];
        while pointer < diffs.len() {
            if diffs[pointer].operation == 1 {
                count_insert += 1;
                text_insert += diffs[pointer].text.as_str();
            }
            else if diffs[pointer].operation == -1 {
                count_delete += 1;
                text_delete += diffs[pointer].text.as_str();
            }
            else {
                if count_delete >= 1 && count_insert >= 1 {
                    let sub_diff = self.diff_main(text_delete.as_str(), text_insert.as_str(),false);
                    for z in sub_diff {
                        temp.push(z);
                    }
                    temp.push(Diff::new(diffs[pointer].operation, diffs[pointer].text.clone()));
                }
                else {
                    if text_delete.is_empty() == false {
                        temp.push(Diff::new(-1, text_delete));
                    }
                    if text_insert.is_empty() == false {
                        temp.push(Diff::new(1, text_insert));
                    }
                    temp.push(Diff::new(diffs[pointer].operation, diffs[pointer].text.clone()));
                }
                count_delete = 0;
                count_insert = 0;
                text_delete = "".to_string();
                text_insert = "".to_string();
            }
            pointer += 1;
        }
        temp.pop();
        temp
    }
    pub fn diff_bisect(&mut self, char1: &Vec<char>, char2: &Vec<char>) -> Vec<Diff> {
        let text1_length = char1.len() as i32;
        let text2_length = char2.len() as i32;
        let max_d: i32 = (text1_length + text2_length + 1)/2;
        let v_offset: i32 = max_d;
        let v_length: i32 = 2 * max_d;
        let mut v1: Vec<i32> = vec![-1; v_length as usize];
        let mut v2: Vec<i32> = vec![-1; v_length as usize];
        v1[v_offset as usize + 1] = 0;
        v2[v_offset as usize + 1] = 0;
        let delta: i32 = text1_length - text2_length;
        let front: i32 = (delta%2 != 0) as i32;
        let mut k1start: i32 = 0;
        let mut k1end: i32 = 0;
        let mut k2start: i32 = 0;
        let mut k2end: i32 = 0;
        for d in 0..max_d {
            let d1 = d as i32;
            let mut k1 = -d1 + k1start;
            let mut x1: i32;
            let mut k1_offset: i32;
            let mut k2_offset;
            let mut x2;
            let mut y1;
            while k1 < d1 + 1 - k1end {
                k1_offset = v_offset + k1;
                if k1 == -d1 || (k1 != d1 && v1[k1_offset as usize - 1] < v1[k1_offset as usize + 1]) {
                    x1 = v1[k1_offset as usize + 1];
                }
                else {
                    x1 = v1[k1_offset as usize - 1] + 1;
                }
                y1 = x1 - k1;
                while x1 < text1_length && y1 < text2_length {
                    let i1;
                    let i2;
                    if x1 < 0 {
                        i1 = text1_length + x1;
                    }
                    else {
                        i1 = x1;
                    }
                    if y1 < 0 {
                        i2 = text2_length + y1;
                    }
                    else {
                        i2 = y1;
                    }
                    if char1[i1 as usize] != char2[i2 as usize] {
                        break;
                    }
                    x1 += 1;
                    y1 += 1;
                }
                v1[k1_offset as usize] = x1;
                if x1 > text1_length {
                    k1end += 2;
                }
                else if y1 > text2_length {
                    k1start += 2;
                }
                else if front != 0 {
                    k2_offset = v_offset + delta - k1;
                    if k2_offset >= 0 && k2_offset < v_length && v2[k2_offset as usize] != -1 {
                        x2 = text1_length - v2[k2_offset as usize];
                        if x1 >= x2 {
                            return self.diff_bisect_split(char1, char2, x1, y1);
                        }
                    }
                }
                k1 += 2;
            }
            let mut k2 = -d1 + k2start;
            let mut y2;
            while k2 < d1 + 1 - k2end {
                k2_offset = v_offset + k2;
                if k2 == -d1 || (k2 != d1 && v2[k2_offset as usize - 1] < v2[k2_offset as usize + 1]) {
                    x2 = v2[k2_offset as usize + 1];
                }
                else {
                    x2 = v2[k2_offset as usize - 1] + 1;
                }
                y2 = x2 - k2;
                while x2 < text1_length && y2 < text2_length {
                    let i1;
                    let i2;
                    if text1_length - x2 - 1 >= 0 {
                        i1 = text1_length - x2 - 1;
                    }
                    else {
                        i1 = x2 + 1;
                    }
                    if text2_length - y2 - 1 >= 0 {
                        i2 = text2_length -y2 - 1; 
                    }
                    else {
                        i2 = y2 + 1;
                    }
                    if char1[i1 as usize] != char2[i2 as usize] {
                        break;
                    }
                    x2 += 1;
                    y2 += 1;
                }
                v2[k2_offset as usize] = x2;
                if x2 > text1_length {
                    k2end += 2;
                }
                else if y2 > text2_length {
                    k2start += 2;
                }
                else if front == 0 {
                    k1_offset = v_offset + delta - k2;
                    if k1_offset >= 0 && k1_offset < v_length && v1[k1_offset as usize] != -1 {
                        x1 = v1[k1_offset as usize];
                        y1 = v_offset + x1 - k1_offset;
                        x2 = text1_length - x2;
                        if x1 >= x2 {
                            return self.diff_bisect_split(char1, char2, x1, y1);
                        }
                    }
                }
                k2 += 2;
            }
        }
        vec![Diff::new(-1, char1.iter().collect()), Diff::new(1, char2.iter().collect())]
    }

    fn diff_bisect_split(&mut self, text1: &Vec<char>, text2: &Vec<char>, x: i32, y: i32) -> Vec<Diff> {
        let text1a: String = text1[..(x as usize)].iter().collect();
        let text2a: String = text2[..(y as usize)].iter().collect();
        let text1b: String = text1[(x as usize)..].iter().collect();
        let text2b: String = text2[(y as usize)..].iter().collect();
        let mut diffs = self.diff_main(text1a.as_str(), text2a.as_str(), false);
        let mut diffsb = self.diff_main(text1b.as_str(), text2b.as_str(), false);
        diffs.append(&mut diffsb);
        diffs
    }
    
    pub fn diff_lines_tochars(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> (String, String, Vec<String>) {
        let mut linearray: Vec<String> = vec!["".to_string()];
        let mut linehash: HashMap<String, i32> = HashMap::new();
        let chars1 = self.diff_lines_tochars_munge(text1, &mut linearray, &mut linehash);
        let mut dmp = Dmp::new();
        let chars2 = dmp.diff_lines_tochars_munge(text2, &mut linearray, &mut linehash);
        (chars1, chars2, linearray)
    }

    pub fn diff_lines_tochars_munge(&mut self, text: &Vec<char>, linearray: &mut Vec<String>, linehash: &mut HashMap<String, i32>) -> String {
        let mut chars = "".to_string();
        let mut line_start = 0;
        let mut line_end = -1;
        let mut line: String;
        while line_end < (text.len() as i32 - 1) {
            line_end = find_char('\n', &text, line_start as usize);
            if line_end == -1 {
                line_end = text.len() as i32 - 1;
            }
            line = text[(line_start as usize)..(line_end as usize + 1)].iter().collect();
            if linehash.contains_key(&line) {
                match char::from_u32(linehash[&line] as u32) {
                    Some(char1) => {
                        chars.push(char1);
                        line_start = line_end + 1;
                    }
                    None => {

                    }
                }
            }
            else {
                if linearray.len() == 1114111 {
                    line = text[(line_start as usize)..].iter().collect();
                    line_end = text.len() as i32 - 1;
                }
                line_start = line_end + 1;
                linearray.push(line.clone());
                linehash.insert(line.clone(), linearray.len() as i32 - 1);
                match char::from_u32(linehash[&line] as u32) {
                    Some(char1) => {
                        chars.push(char1);
                        line_start = line_end + 1;
                    }
                    None => {

                    }
                }
            }
        }
        chars
    }

    pub fn diff_chars_tolines(&mut self, diffs: &mut Vec<Diff>, line_array: &Vec<String> ) {
        let len = diffs.len();
        for i in 0..len {
            let mut text: String = "".to_string();
            let text1 = diffs[i].text.clone();
            let chars: Vec<char> = text1.chars().collect();
            for j in 0..chars.len() {
                text += line_array[chars[j] as usize].as_str();
            }
            diffs[i].text = text;
        }
    }

    pub fn diff_common_prefix(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> i32 {
        if text1.is_empty() || text2.is_empty() {
            return 0;
        }
        let pointermax = min(text1.len() as i32, text2.len() as i32);
        let mut pointerstart = 0;
        while pointerstart < pointermax {
            if text1[pointerstart as usize] == text2[pointerstart as usize] {
                pointerstart += 1;
            }
            else {
                return pointerstart as i32;
            }
        }
        pointermax
    }

    pub fn diff_common_suffix(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> i32 {
        if text1.is_empty() || text2.is_empty() {
            return 0;
        }
        let mut pointer_1 = text1.len() as i32 - 1;
        let mut pointer_2 = text2.len() as i32 - 1;
        let mut len = 0;
        while pointer_1 >= 0 && pointer_2 >= 0 {
            if text1[pointer_1 as usize] == text2[pointer_2 as usize] {
                len += 1;
            }
            else {
                break;
            }
            pointer_1 -= 1;
            pointer_2 -= 1;
        }
        len
    }

    pub fn diff_common_overlap(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> i32 {
        let text1_length = text1.len();
        let text2_length = text2.len();
        if text1_length == 0 || text2_length == 0 {
            return 0;
        }
        let mut text1_trunc;
        let mut text2_trunc;
        let len = min(text1_length as i32, text2_length as i32);

        if text1.len() > text2.len() {
            text1_trunc = text1[(text1_length - text2_length)..].to_vec();
            text2_trunc = text2[..].to_vec();
        }
        else {
            text1_trunc = text1[..].to_vec();
            text2_trunc = text2[0..text1_length].to_vec();
        }
        let mut best = 0;
        let mut length = 1;
        
        if text1_trunc == text2_trunc {
            return len;
        }
        loop {
            let patern = text1_trunc[(len as usize - length)..(len as usize)].to_vec();
            let found = self.kmp(&text2_trunc, &patern, 0);
            if found == -1 {
                return best;
            }
            length += found as usize;
            if found == 0 {
                best = length as i32;
                length += 1;
            }
        }
    }


    #[allow(dead_code)]
    pub fn split_by_char(&mut self, text: &str, ch: char) -> Vec<String> {
        let temp: Vec<&str> = text.split(ch).collect();
        let mut temp1: Vec<String> = vec![];
        for i in 0..temp.len() {
            temp1.push(temp[i].to_string());
        }
        temp1
    }

    #[allow(dead_code)]
    pub fn split_by_chars(&mut self, text: &str) -> Vec<String> {
        let temp: Vec<&str> = text.split("@@ ").collect();
        let mut temp1: Vec<String> = vec![];
        for i in 0..temp.len() {
            temp1.push(temp[i].to_string());
        }
        temp1
    }

    pub fn diff_half_match(&mut self, text1: &Vec<char>, text2: &Vec<char>) -> Vec<String> {
        let long_text;
        let short_text;
        if text1.len() > text2.len() {
            long_text = text1;
            short_text = text2;
        }
        else {
            long_text = text2;
            short_text = text1;
        }
        let len1 = short_text.len();
        let len2 = long_text.len();
        if len2 < 4 || len1*2 <len2 {
            return vec![];
        }
        let mut hm: Vec<String>;
        let hm1 = self.diff_half_matchi(long_text, short_text, (len2 as i32 + 3)/4);
        let hm2 = self.diff_half_matchi(long_text, short_text, (len2 as i32 + 1)/2);
        
        if hm1.is_empty() && hm2.is_empty() {
            return vec![];
        }
        else if hm1.is_empty() {
            hm = hm2;
        }
        else if hm2.is_empty() {
            hm = hm1;
        }
        else {
            if hm1[4].len() > hm2[4].len() {
                hm = hm1;
            }
            else {
                hm = hm2;
            }
        }
        if text1.len() > text2.len() {
            return hm;
        }
        let mut temp2 = hm[0].clone();
        let mut temp3 = hm[2].clone();
        hm[0] = temp3;
        hm[2] = temp2;
        temp2 = hm[1].clone();
        temp3 = hm[3].clone();
        hm[1] = temp3;
        hm[3] = temp2;
        return hm;
    }

    fn diff_half_matchi(&mut self, long_text: &Vec<char>, short_text: &Vec<char>, i: i32) -> Vec<String> {
        let long_len = long_text.len();
        let seed = Vec::from_iter(long_text[(i as usize)..(i as usize + long_len / 4)].iter().cloned());
        let mut best_common = "".to_string();
        let mut best_longtext_a = "".to_string();
        let mut best_longtext_b = "".to_string();
        let mut best_shorttext_a = "".to_string();
        let mut best_shorttext_b = "".to_string();
        let mut j: i32 = self.kmp(short_text, &seed, 0);
        while j != -1 {
            let prefix_length = self.diff_common_prefix(&long_text[(i as usize)..].to_vec(), &short_text[(j as usize)..].to_vec());
            let suffix_length = self.diff_common_suffix(&long_text[..(i as usize)].to_vec(), &short_text[..(j as usize)].to_vec());
            if best_common.len() < suffix_length as usize + prefix_length as usize {
                best_common = short_text[(j as usize - suffix_length as usize)..(j as usize + prefix_length as usize)].iter().collect();
                best_longtext_a = long_text[..((i - suffix_length) as usize)].iter().collect();
                best_longtext_b = long_text[((i + prefix_length) as usize)..].iter().collect();
                best_shorttext_a = short_text[..((j - suffix_length) as usize)].iter().collect();
                best_shorttext_b = short_text[((j + prefix_length) as usize)..].iter().collect();
            }
            j = self.kmp(short_text, &seed, j as usize + 1);
        }
        if best_common.chars().count() * 2 >= long_text.len() {
            return vec![best_longtext_a, best_longtext_b, best_shorttext_a, best_shorttext_b, best_common];
        }
        vec![]
    }
    pub fn diff_cleanup_semantic(&mut self, diffs: &mut Vec<Diff>) {
        let mut changes = false;
        let mut equalities: Vec<i32> = vec![];
        let mut last_equality = "".to_string();
        let mut pointer: i32 = 0;
        let mut length_insertions1 = 0;
        let mut length_insertions2 = 0;
        let mut length_deletions1 = 0;
        let mut length_deletions2 = 0;
        while (pointer as usize) < diffs.len() {
            if diffs[pointer as usize].operation == 0 {
                equalities.push(pointer);
                length_insertions1 = length_insertions2;
                length_insertions2 = 0;
                length_deletions1 = length_deletions2;
                length_deletions2 = 0;
                last_equality = diffs[pointer as usize].text.clone();
            }
            else {
                if diffs[pointer as usize].operation == 1 {
                    length_insertions2 += diffs[pointer as usize].text.chars().count() as i32;
                }
                else {
                    length_deletions2 += diffs[pointer as usize].text.chars().count() as i32;
                }
                let last_equality_len = last_equality.chars().count() as i32;
                if last_equality_len > 0 && last_equality_len <= max(length_insertions1, length_deletions1) && 
                                            last_equality_len <= max(length_insertions2, length_deletions2) {
                    diffs.insert(equalities[equalities.len() - 1] as usize, Diff::new(-1, last_equality.clone()));
                    diffs[equalities[equalities.len() - 1] as usize + 1] = Diff::new(1, diffs[equalities[equalities.len() - 1] as usize + 1].text.clone());
                    equalities.pop();
                    if equalities.len() > 0 {
                        equalities.pop();
                    }
                    if equalities.len() > 0 {
                        pointer = equalities[equalities.len() - 1];
                    }
                    else {
                        pointer = -1;
                    }
                    length_insertions1 = 0;
                    length_deletions1 = 0;
                    length_insertions2 = 0;
                    length_deletions2 = 0;
                    last_equality = "".to_string();
                    changes = true;
                }
            }
            pointer += 1;
        }
        if changes {
            self.diff_cleanup_merge(diffs);
        }
        let mut overlap_length1: i32;
        let mut overlap_length2: i32;
        self.diff_cleanup_semantic_lossless(diffs);
        pointer = 1;
        while (pointer as usize) < diffs.len() {
             if diffs[pointer as usize - 1].operation == -1 && diffs[pointer as usize].operation == 1 {
                // deletion = diffs[pointer as usize - 1].text.clone();
                let deletion_vec: Vec<char> = diffs[pointer as usize - 1].text.chars().collect();
                let insertion_vec: Vec<char> = diffs[pointer as usize].text.chars().collect();
                // insertion = diffs[pointer as usize].text.clone();
                overlap_length1 = self.diff_common_overlap(&deletion_vec, &insertion_vec);
                overlap_length2 = self.diff_common_overlap(&insertion_vec, &deletion_vec);
                if overlap_length1 >= overlap_length2 {
                    if (overlap_length1 as f32) >= (deletion_vec.len() as f32 / 2.0) || (overlap_length1 as f32) >= (insertion_vec.len() as f32 / 2.0)
                    {
                        diffs.insert(pointer as usize, Diff::new(0, insertion_vec[..(overlap_length1 as usize)].iter().collect()));
                        diffs[pointer as usize - 1] = Diff::new(-1, deletion_vec[..(deletion_vec.len() - overlap_length1 as usize)].iter().collect());
                        diffs[pointer as usize + 1] = Diff::new(1, insertion_vec[(overlap_length1 as usize)..].iter().collect());
                        pointer += 1;
                    }
                }
                else {
                    if (overlap_length2 as f32) >= (deletion_vec.len() as f32 / 2.0) || (overlap_length2 as f32) >= (insertion_vec.len() as f32 / 2.0){
                        diffs.insert(pointer as usize, Diff::new(0, deletion_vec[..(overlap_length2 as usize)].iter().collect()));
                       let insertion_vec_len = insertion_vec.len();
                        diffs[pointer as usize - 1] = Diff::new(1, insertion_vec[..(insertion_vec_len - overlap_length2 as usize)].iter().collect());
                        diffs[pointer as usize + 1] = Diff::new(-1, deletion_vec[(overlap_length2 as usize)..].iter().collect());
                        pointer += 1;
                    }
                }
                pointer += 1;
            }
            pointer += 1; 
        } 
    }

    pub fn diff_cleanup_semantic_lossless(&mut self, diffs: &mut Vec<Diff>) {
        let mut pointer = 1;
        let mut equality1;
        let mut equality2;
        let mut edit: String;
        let mut common_offset;
        let mut common_string: String;
        let mut best_equality1;
        let mut best_edit;
        let mut best_equality2;
        let mut best_score;
        let mut score;
        while pointer < diffs.len() as i32 - 1 {
            if diffs[pointer as usize - 1].operation == 0 && diffs[pointer as usize + 1].operation == 0 {
                equality1 = diffs[pointer as usize - 1].text.clone();
                edit = diffs[pointer as usize].text.clone();
                equality2 = diffs[pointer as usize + 1].text.clone();
                let mut edit_vec: Vec<char> = edit.chars().collect();
                let mut equality1_vec: Vec<char> = equality1.chars().collect();
                let mut equality2_vec: Vec<char> = equality2.chars().collect();
                common_offset = self.diff_common_suffix(&equality1_vec, &edit_vec);
                if common_offset != 0 {
                    common_string = edit_vec[(edit_vec.len() - common_offset as usize)..].iter().collect();
                    equality1 = equality1_vec[..(equality1_vec.len() - common_offset as usize)].iter().collect();
                    let temp7: String = edit_vec[..(edit_vec.len() - common_offset as usize)].iter().collect();
                    edit = common_string.clone() + temp7.as_str();
                    equality2 = common_string + equality2.as_str();
                    edit_vec = edit.chars().collect();
                    equality2_vec = equality2.chars().collect();
                    equality1_vec = equality1.chars().collect();
                }
                best_equality1 = equality1.clone();
                best_edit = edit;
                best_equality2 = equality2;

                best_score = self.diff_cleanup_semantic_score(&equality1_vec, &edit_vec) + self.diff_cleanup_semantic_score(&edit_vec, &equality2_vec);
                let edit_len = edit_vec.len();
                let mut equality2_len = equality2_vec.len();
                while equality2_len > 0 && edit_len > 0 {
                    if edit_vec[0] != equality2_vec[0] {
                        break;
                    }
                    let ch = edit_vec[0];
                    equality1_vec.push(ch);
                    edit_vec.push(ch);
                    edit_vec = edit_vec[1..].to_vec();
                    equality2_len -= 1;
                    equality2_vec = equality2_vec[1..].to_vec();
                    score = self.diff_cleanup_semantic_score(&equality1_vec, &edit_vec) + self.diff_cleanup_semantic_score(&edit_vec, &equality2_vec);
                    if score >= best_score {
                        best_score = score;
                        best_equality1 = equality1_vec[0..].iter().collect();
                        best_edit = edit_vec[..].iter().collect();
                        best_equality2 = equality2_vec[..].iter().collect();
                    }
                }
                if diffs[pointer as usize - 1].text != best_equality1 {
                    if best_equality1.is_empty() == false {
                        diffs[pointer as usize - 1] = Diff::new(diffs[pointer as usize - 1].operation, best_equality1);
                    }
                    else {
                        diffs.remove(pointer as usize - 1);
                        pointer -= 1;
                    }
                    diffs[pointer as usize] = Diff::new(diffs[pointer as usize].operation, best_edit);
                    if best_equality2.is_empty() == false {
                        diffs[pointer as usize + 1] = Diff::new(diffs[pointer as usize + 1].operation, best_equality2);
                    }
                    else {
                        diffs.remove(pointer as usize + 1);
                        pointer += 1;
                    }
                }
            }
            pointer += 1;
        }
    }

    fn diff_cleanup_semantic_score(&mut self, one: &Vec<char>, two: &Vec<char>) -> i32 {
        if one.is_empty() || two.is_empty() {
            return 6;
        }
        let char1 = one[one.len() - 1];
        let char2 = two[0];
        let nonalphanumeric1: bool = char1.is_alphanumeric() == false;
        let nonalphanumeric2: bool = char2.is_alphanumeric() == false;
        let whitespace1: bool = nonalphanumeric1 & char1.is_whitespace();
        let whitespace2: bool = nonalphanumeric2 & char2.is_whitespace();
        let linebreak1: bool = whitespace1 & ((char1 == '\r') | (char1 == '\n'));
        let linebreak2: bool = whitespace2 & ((char2 == '\r') | (char2 == '\n'));
        let mut test1: bool = false;
        let mut test2: bool = false;
        if one.len() > 1 && one[one.len() - 1] == '\n' && one[one.len() - 2] == '\n' {
            test1 = true;
        }
        if one.len() > 2 && one[one.len() - 1] == '\n' && one[one.len() - 3] == '\n' && one[one.len() - 2] == '\r' {
            test1 = true;
        }
        if two.len() > 1 && two[two.len() - 1] == '\n' && two[two.len() - 2] == '\n' {
            test2 = true;
        }
        if two.len() > 2 && two[two.len() - 1] == '\n' && two[two.len() - 3] == '\n' && two[two.len() - 2] == '\r' {
            test2 = true;
        }
        let blankline1: bool = linebreak1 & test1;
        let blankline2: bool = linebreak2 & test2;
        if blankline1 || blankline2 {
            return 5;
        }
        if linebreak1 || linebreak2
        {
            return 4;
        }
        if nonalphanumeric1 && !whitespace1 && whitespace2
        {
            return 3;
        }
        if whitespace1 || whitespace2
        {
            return 2;
        }
        if nonalphanumeric1 || nonalphanumeric2
        {
            return 1;
        }
        0
    }

    pub fn diff_cleanup_efficiency(&mut self, diffs: &mut Vec<Diff> ) {
        if diffs.is_empty() {
            return;
        }
        let mut changes: bool = false;
        let mut equalities: Vec<i32> = vec![];
        let mut last_equality: String = "".to_string();
        let mut pointer: i32 = 0;
        let mut pre_ins = false;
        let mut pre_del = false;
        let mut post_ins = false;
        let mut post_del = false;
        while (pointer as usize) < diffs.len() {
            if diffs[pointer as usize].operation == 0 {
                if diffs[pointer as usize].text.chars().count() < self.edit_cost as usize && (post_del || post_ins) {
                    equalities.push(pointer);
                    pre_ins = post_ins;
                    pre_del = post_del;
                    last_equality = diffs[pointer as usize].text.clone();
                }
                else {
                    equalities = vec![];
                    last_equality = "".to_string();
                }
                post_ins = false;
                post_del = false;
            }
            else {
                if diffs[pointer as usize].operation == -1 {
                    post_del = true;
                }
                else {
                    post_ins = true;
                }
                if last_equality.is_empty() == false && ((pre_ins && pre_del && post_del && post_ins) ||
                    ((last_equality.chars().count() as i32) < self.edit_cost / 2 && 
                    (pre_ins as i32 + pre_del as i32 + post_del as i32 + post_ins as i32) == 3)) {
                    
                    diffs.insert(equalities[equalities.len() - 1] as usize, Diff::new(-1, last_equality));
                    diffs[equalities[equalities.len() - 1] as usize + 1] = Diff::new(1, diffs[equalities[equalities.len() - 1] as usize + 1].text.clone());
                    equalities.pop();
                    last_equality = "".to_string();
                    if pre_ins && pre_del {
                        post_del = true;
                        post_ins = true;
                        equalities = vec![];
                    }
                    else {
                        if equalities.len() > 0 {
                            equalities.pop();
                        }
                        if equalities.len() > 0 {
                            pointer = equalities[equalities.len() - 1];
                        }
                        else {
                            pointer = -1;
                        }
                        post_ins = false;
                        post_del = false;
                    }
                    changes = true;
                }
            }
            pointer += 1;
        }
        if changes {
            self.diff_cleanup_merge(diffs);
        }
    }

    pub fn diff_cleanup_merge(&mut self, diffs: &mut Vec<Diff>) {
        if diffs.is_empty() {
            return;
        }
        diffs.push(Diff::new(0, "".to_string()));
        let mut text_insert: String = "".to_string();
        let mut text_delete: String = "".to_string();
        let mut i: i32 = 0;
        let mut count_insert = 0;
        let mut count_delete = 0;
        while (i as usize) < diffs.len() {
            if diffs[i as usize].operation == -1 {
                text_delete += diffs[i as usize].text.as_str();
                count_delete += 1;
                i += 1; 
            }
            else if diffs[i as usize].operation == 1 {
                text_insert += diffs[i as usize].text.as_str();
                count_insert += 1;
                i += 1;
            }
            else {
                if count_delete + count_insert > 1 {
                    let mut delete_vec: Vec<char> = text_delete.chars().collect();
                    let mut insert_vec: Vec<char> = text_insert.chars().collect();
                    if count_delete > 0 && count_insert > 0 {
                        let mut commonlength = self.diff_common_prefix(&insert_vec, &delete_vec);
                        if commonlength != 0 {
                            let temp1: String = (&insert_vec)[..(commonlength as usize)].iter().collect();
                            let x = i - count_delete - count_insert - 1;
                            if x >= 0 && diffs[x as usize].operation == 0 {
                                diffs[x as usize] = Diff::new(diffs[x as usize].operation, diffs[x as usize].text.clone() + temp1.as_str());
                            }
                            else {
                                diffs.insert(0, Diff::new(0, temp1));
                                i += 1;
                            }
                            insert_vec = insert_vec[(commonlength as usize)..].to_vec();
                            delete_vec = delete_vec[(commonlength as usize)..].to_vec();
                        }
                        commonlength = self.diff_common_suffix(&insert_vec, &delete_vec);
                        if commonlength != 0 {
                            let temp1: String = (&insert_vec)[(insert_vec.len() - commonlength as usize)..].iter().collect();
                            diffs[i as usize] = Diff::new(diffs[i as usize].operation, temp1 + diffs[i as usize].text.as_str());
                            insert_vec = insert_vec[..(insert_vec.len() - commonlength as usize)].to_vec();
                            delete_vec = delete_vec[..(delete_vec.len() - commonlength as usize)].to_vec();
                        }
                    }
                    i -= count_delete + count_insert;
                    for _j in 0..(count_delete + count_insert) as usize {
                        diffs.remove(i as usize);
                    }
                    if delete_vec.len() > 0 {
                        diffs.insert(i as usize, Diff::new(-1, delete_vec.iter().collect()));
                        i += 1;
                    }
                    if insert_vec.len() > 0 {
                        diffs.insert(i as usize, Diff::new(1, insert_vec.iter().collect()));
                        i+= 1;
                    }
                    i += 1;
                }
                else if i != 0 && diffs[i as usize - 1].operation == 0 {
                    diffs[i as usize - 1] = Diff::new(diffs[i as usize - 1].operation, diffs[i as usize - 1].text.clone() + diffs[i as usize].text.as_str());
                    diffs.remove(i as usize);
                }
                else {
                    i += 1;
                }
                count_delete = 0;
                text_delete = "".to_string();
                text_insert = "".to_string();
                count_insert = 0;
            }
        }
        if diffs[diffs.len() - 1].text == "".to_string() {
            diffs.pop();
        }
        let mut changes = false;
        i = 1;
        while (i as usize) < diffs.len() - 1 {
            if diffs[i as usize -1].operation == 0 && diffs[i as usize + 1].operation == 0 {
                let text_vec = diffs[i as usize].text.chars().collect();
                let text1_vec = diffs[i as usize - 1].text.chars().collect();
                let text2_vec: Vec<char> = diffs[i as usize + 1].text.chars().collect();
                if self.endswith(&text_vec, &text1_vec) {
                    if diffs[i as usize - 1].text != "".to_string() {
                        let temp1: String = diffs[i as usize - 1].text.clone();
                        let temp2: String = text_vec[..(text_vec.len() - text1_vec.len())].iter().collect();
                        diffs[i as usize].text = temp1 + temp2.as_str();
                        diffs[i as usize + 1].text = diffs[i as usize - 1].text.clone() + diffs[i as usize + 1].text.as_str();
                    }
                    diffs.remove(i as usize - 1);
                    changes = true;
                }
                else if self.startswith(&text_vec, &text2_vec) {
                    diffs[i as usize - 1].text = diffs[i as usize - 1].text.clone() + diffs[i as usize + 1].text.as_str();
                    let temp1: String = text_vec[text2_vec.len()..].iter().collect();
                    diffs[i as usize].text = temp1 + diffs[i as usize + 1].text.as_str();
                    diffs.remove(i as usize + 1);
                    changes = true;
                }
            }
            i += 1;
        }
        if changes {
            self.diff_cleanup_merge(diffs);
        }
    }

    fn endswith(&mut self, first: &Vec<char>, second: &Vec<char>) -> bool {
        let mut len1 = first.len();
        let mut len2 = second.len();
        if len1 < len2 {
            return false;
        }
        while len2 > 0 {
            if first[len1 - 1] != second[len2 - 1] {
                return false;
            }
            len1 -= 1;
            len2 -= 1;
        }
        return true;
    }

    fn startswith(&mut self, first: &Vec<char>, second: &Vec<char>) -> bool {
        let len1 = first.len();
        let len2 = second.len();
        if len1 < len2 {
            return false;
        }
        for i in 0..len2 {
            if first[i] != second[i] {
                return false;
            }
        }
        true
    }

    #[allow(dead_code)]
    pub fn diff_xindex(&mut self, diffs: &[Diff], loc: i32) -> i32 {
        let mut chars1 = 0;
        let mut chars2 = 0;
        let mut last_chars1 = 0;
        let mut last_chars2 = 0;
        let mut lastdiff = Diff::new(0, "".to_string());
        let mut z = 0;
        for diffs_item in diffs {
            if diffs_item.operation != 1 {
                chars1 += diffs_item.text.chars().count() as i32;
            }
            if diffs_item.operation != -1 {
                chars2 += diffs_item.text.chars().count() as i32;
            }
            if chars1 > loc {
                lastdiff = Diff::new(diffs_item.operation, diffs_item.text.clone());
                break;
            }
            last_chars1 = chars1;
            last_chars2 = chars2;
        }
        if lastdiff.operation == -1 && diffs.len() != z {
            return last_chars2;
        }
        last_chars2 + (loc - last_chars1)
    }

    #[allow(dead_code)]
    pub fn diff_text1(&mut self, diffs: &mut Vec<Diff>) -> String {
        let mut text: String = "".to_string();
        for adiff in diffs {
            if adiff.operation != 1 {
                text += adiff.text.as_str();
            }
        }
        text
    }

    #[allow(dead_code)]
    pub fn diff_text2(&mut self, diffs: &mut Vec<Diff>) -> String {
        let mut text: String = "".to_string();
        for adiff in diffs {
            if adiff.operation != -1 {
                text += adiff.text.as_str();
            }
        }
        text
    }

    #[allow(dead_code)]
    pub fn diff_levenshtein(&mut self, diffs: &[Diff]) -> i32 {
        let mut levenshtein = 0;
        let mut insertions = 0;
        let mut deletions = 0;
        for adiff in diffs {
            if adiff.operation == 1 {
                insertions += adiff.text.chars().count();
            }
            else if adiff.operation == -1 {
                deletions += adiff.text.chars().count();
            }
            else {
                levenshtein += max(insertions as i32, deletions as i32);
                insertions = 0;
                deletions = 0;
            }
        }
        levenshtein += max(insertions as i32, deletions as i32);
        return levenshtein;
    }

    #[allow(dead_code)]
    pub fn diff_todelta(&mut self, diffs: &mut Vec<Diff>) -> String {
        let mut text: String = "".to_string();
        let len = diffs.len();
        for (k, diffs_item) in diffs.iter().enumerate().take(len) {
            if diffs_item.operation == 1 {
                let temp5: Vec<char> = vec!['!', '~', '*', '(', ')', ';', '/', '?', ':', '@', '&', '=', '+', '$', ',', '#', ' ', '\''];
                let temp4: Vec<char> = diffs_item.text.chars().collect();
                text += "+";
                let mut text1 = "".to_string();
                for temp4_item in &temp4 {
                    let mut is = false;
                    for temp5_item in &temp5 {
                        if *temp5_item == *temp4_item {
                            text.push(*temp4_item);
                            text1.push(*temp4_item);
                            is = true;
                            break;
                        }
                    }
                    if is {
                        continue;
                    }
                    let mut temp6 = "".to_string();
                    temp6.push(*temp4_item);
                    temp6 = utf8_percent_encode(temp6.as_str(), USERINFO_ENCODE_SET).collect();
                    text += temp6.as_str();
                    text1 += temp6.as_str();
                }
                // println!("{} {:?}", text1,temp4);
            }
            else if diffs_item.operation == -1 {
                let temp4: String = utf8_percent_encode(diffs_item.text.chars().count().to_string().as_str(), DEFAULT_ENCODE_SET).collect();
                text += "-";
                text += temp4.as_str();
            }
            else {
                let temp4: String = utf8_percent_encode(diffs_item.text.chars().count().to_string().as_str(), DEFAULT_ENCODE_SET).collect();
                text += "=";
                text += temp4.as_str();
            }
            if k < len - 1 {
                text += "\t";
            }
        }
        text
    }

    #[allow(dead_code)]
    pub fn diff_from_delta(&mut self, text1: &str, delta: &str) -> Vec<Diff> {
        let mut diffs: Vec<Diff> = vec![];
        let tokens: Vec<&str> = (*delta).split('\t').collect();
        let text1_vec: Vec<char> = text1.chars().collect();
        let len = text1.chars().count();
        let mut text_len = 0;
        for token in tokens {
            if token =="" {
                continue;
            }
            let token_vec: Vec<char> = token.chars().collect();
            let operation: String = (&token_vec)[0..1].iter().collect();
            let text: String = (&token_vec)[1..].iter().collect();
            let text2 = percent_decode(text.as_bytes()).decode_utf8().unwrap().to_string();
            if operation.as_str() == "+" {
                diffs.push(Diff::new(1, text2));
            }
            else if operation.as_str() == "=" {
                let str_size = text2.as_str().parse::<i32>().unwrap();
                if str_size as usize + text_len > len {
                    panic!("wrong patern or text");
                }
                // println!("{}", len);
                let temp8: String = (&text1_vec)[max(text_len as i32, 0) as usize..min(str_size + text_len as i32, text1_vec.len() as i32) as usize].iter().collect();
                // println!("{}", token);
                diffs.push(Diff::new(0, temp8));
                text_len += str_size as usize;
            }
            else{
                
                let str_size = text2.as_str().parse::<i32>().unwrap();
                if str_size as usize + text_len > len {
                    panic!("wrong patern or text");
                }
                let temp8: String = (&text1_vec)[max(text_len as i32, 0)as usize..min(text1_vec.len() as i32, text_len as i32 + str_size as i32) as usize].iter().collect();
                diffs.push(Diff::new(-1, temp8));
                text_len += str_size as usize;
            }
        }
        if len != text_len {
            panic!("wrong patern or text");
        }
        diffs
    } 

    #[allow(dead_code)]
    pub fn match_main(&mut self, text1: &str, patern1: &str, mut loc: i32) -> i32 {
        loc  = max(0, min(loc, text1.len() as i32));
        if patern1.is_empty() {
            return loc;
        }
        if text1.is_empty() {
            return -1;
        }
        let text: Vec<char> = (text1.to_string()).chars().collect();
        let patern: Vec<char> = (patern1.to_string()).chars().collect();
        if text == patern {
            return 0;
        }
        else if loc as usize + patern.len() <= text.len() && text[(loc as usize)..(loc as usize + patern.len())].to_vec() == patern {
            return loc;
        }
        return self.match_bitap(&text, &patern, loc);
    }

    #[allow(dead_code)]
    pub fn match_bitap(&mut self, text: &[char], patern: &[char], loc: i32) -> i32 {
        if !(self.match_maxbits == 0 || patern.len() as i32 <= self.match_maxbits) {
            panic!("patern too long for this application");
        }
        let s: HashMap<char, i32> = self.match_alphabet(patern);
        let mut score_threshold: f32 = self.match_threshold;
        let mut best_loc = self.kmp(text, patern, loc as usize);
        if best_loc != -1 {
            score_threshold = min1(self.match_bitap_score(0, best_loc, loc, patern), score_threshold);
            best_loc = self.rkmp(text, patern, loc as usize + patern.len());
            if best_loc != -1 {
                score_threshold = min1(score_threshold, self.match_bitap_score(0, best_loc, loc, patern));
            }
        }
        let matchmask = 1 << (patern.len() - 1);//>
        best_loc = -1;
        let mut bin_min: i32;
        let mut bin_mid: i32;
        let mut bin_max: i32 = (patern.len() + text.len()) as i32;
        let mut last_rd: Vec<i32> = vec![];
        for d in 0..patern.len() {
            let mut rd: Vec<i32> = vec![];
            bin_min = 0;
            bin_mid = bin_max;
            while bin_min < bin_mid {
                if self.match_bitap_score(d as i32, loc + bin_mid, loc, patern) <= score_threshold {
                    bin_min = bin_mid;
                }
                else {
                    bin_max = bin_mid;
                }
                bin_mid = bin_min + (bin_max - bin_min) / 2;
            }
            bin_max = bin_mid;
            let mut start = max(1, loc - bin_mid + 1);
            let finish = min(loc + bin_mid, text.len() as i32) + patern.len() as i32;
            rd.resize((finish + 2) as usize, 0);
            rd[(finish + 1) as usize] = (1 << d) - 1;//>
            let mut j = finish;
            while j >= start {
                let char_match: i32;
                if text.len() + 1 <= j as usize {
                    char_match = 0;
                }
                else {
                    match s.get(&(text[j as usize -1])) {
                        Some(num) => {
                            char_match = *num;
                        }
                        None => {
                            char_match = 0;
                        }
                    }
                }
                if d == 0 {
                    rd[j as usize] = ((rd[j as usize + 1] << 1) | 1) & char_match;//>>
                }
                else {
                    rd[j as usize] = (((rd[j as usize + 1] << 1) | 1) & char_match) | (((last_rd[j as usize + 1] | (last_rd[j as usize]) << 1)) | 1) | last_rd[j as usize + 1];//>>>>
                }
                if (rd[j as usize] & matchmask) != 0 {
                    let score: f32 = self.match_bitap_score(d as i32, j - 1, loc, patern);
                    if score <= score_threshold {
                        score_threshold = score;
                        best_loc = j - 1;
                        if best_loc > loc {
                            start = max(1, 2*loc - best_loc);
                        }
                        else {
                            break;
                        }
                    }
                }
                j -= 1;
            }
            if self.match_bitap_score(d as i32 + 1, loc, loc, patern) > score_threshold {
                break;
            }
            last_rd = rd;
        }
        best_loc
    }

    pub fn match_bitap_score(&mut self, e: i32, x: i32, loc: i32, patern: &[char]) -> f32 {
        let accuracy: f32 = (e as f32) /  (patern.len() as f32);
        let proximity: i32 = (loc - x).abs();
        if self.match_distance == 0 {
            if proximity == 0 {
                return accuracy;
            }
            else {
                return 1.0;
            }
        }
        return accuracy + ((proximity as f32) / (self.match_distance as f32));
    }
    pub fn match_alphabet(&mut self, patern: &[char]) -> HashMap<char,i32> {
        let mut s: HashMap<char,i32> = HashMap::new();
        for patern_item in patern {
            s.insert(*patern_item, 0);
        }
        for i in 0..patern.len() {
            let ch: char = patern[i];
            let mut temp: i32 = 0;
            if let Some(num) = s.get(&ch) {
                temp = num|(1 << (patern.len() - i - 1));//>>
            }
            s.insert(ch, temp);
        }
        s
    }

    #[allow(dead_code)]
    pub fn patch_add_context(&mut self, patch: &mut Patch, text: &mut Vec<char>) {
        if text.is_empty() {
            return;
        }
        let mut pattern: Vec<char> = text[patch.start2 as usize..(patch.length1 as usize + patch.start2 as usize)].to_vec();
        let mut padding: i32 = 0;
        let mut rst = 0;
        // println!("{} {}", self.kmp(text, &pattern, 0), self.rkmp(&text, &pattern, text.len() - 1));
        while self.kmp(text, &pattern, 0) != self.rkmp(&text, &pattern, text.len() - 1) && (pattern.len() as i32) < (self.match_maxbits - self.patch_margin * 2) {
            padding += self.patch_margin;
            pattern = text[max(0, patch.start2 - padding) as usize..min(text.len() as i32, patch.start2 + patch.length1 + padding) as usize].to_vec();
            // println!("{} {}", pattern.len(), text.len());
            rst += 1;
            if rst > 5 {
                break;
            }
        }
        // println!("{:?}", pattern);
        padding += self.patch_margin;
        let prefix: String = text[max(0, patch.start2 - padding) as usize..patch.start2 as usize].iter().collect();
        let prefix_length = prefix.chars().count() as i32;
        if !prefix.is_empty() {
            patch.diffs.insert(0, Diff::new(0, prefix.clone()));
        }
        let suffix: String = text[(patch.start2 + patch.length1) as usize..min(text.len() as i32, patch.start2 + patch.length1 + padding) as usize].iter().collect();
        let suffix_length = suffix.chars().count() as i32;
        if !suffix.is_empty() {
            patch.diffs.push(Diff::new(0, suffix));
        }
        patch.start1 -= prefix_length;
        patch.start2 -= prefix_length;
        patch.length1 += prefix_length + suffix_length;
        patch.length2 += prefix_length + suffix_length;
    }

    #[allow(dead_code)]
    pub fn patch_make1(&mut self, text1: &str, text2: &str) -> Vec<Patch> {
        let mut diffs: Vec<Diff> = self.diff_main(text1, text2, true);
        if diffs.len() > 2 {
            self.diff_cleanup_semantic(&mut diffs);
            self.diff_cleanup_efficiency(&mut diffs);
        }
        return self.patch_make4(text1, &mut diffs);
    }

    #[allow(dead_code)]
    pub fn patch_make2(&mut self, diffs: &mut Vec<Diff>) -> Vec<Patch> {
        let text1 = self.diff_text1(diffs);
        return self.patch_make4(text1.as_str(), diffs);
    }

    #[allow(dead_code)]
    pub fn patch_make3(&mut self, text1: &str, _text2: &str, diffs: &mut Vec<Diff>) -> Vec<Patch> {
        return self.patch_make4(text1, diffs);
    }
    pub fn patch_make4(&mut self, text1: &str, diffs: &mut Vec<Diff>) -> Vec<Patch> {
        let mut patches: Vec<Patch> = vec![];
        if diffs.is_empty() {
            return patches;
        }
        let mut patch: Patch = Patch::new(vec![], 0, 0, 0, 0);
        let mut char_count1 = 0;
        let mut char_count2 = 0;
        let mut prepatch: Vec<char> = (text1.to_string()).chars().collect();
        let mut postpatch: Vec<char> = (text1.to_string()).chars().collect();
        for i in 0..diffs.len() {
            let temp1: &Vec<char> = &(diffs[i].text.chars().collect());
            if patch.diffs.is_empty() && diffs[i].operation != 0 {
                patch.start1 = char_count1;
                patch.start2 = char_count2;
            }
            if diffs[i].operation == 1 {
                patch.diffs.push(Diff::new(diffs[i].operation, diffs[i].text.clone()));
                let temp: Vec<char> = postpatch[char_count2 as usize..].to_vec();
                postpatch = postpatch[..char_count2 as usize].to_vec();
                patch.length2 += temp1.len() as i32;
                for ch in temp1 {
                    postpatch.push(*ch);
                }
                for ch in temp {
                    postpatch.push(ch);
                }
            }
            else if diffs[i].operation == -1 {
                patch.diffs.push(Diff::new(diffs[i].operation, diffs[i].text.clone()));
                let temp: Vec<char> = postpatch[(temp1.len() + char_count2 as usize)..].to_vec();
                postpatch = postpatch[..char_count2 as usize].to_vec();
                patch.length1 += temp1.len() as i32;
                for ch in &temp {
                    postpatch.push(*ch);
                }
            }
            else {
                if temp1.len() as i32 <= self.patch_margin * 2 && !patch.diffs.is_empty() && i != diffs.len() - 1 {
                    patch.diffs.push(Diff::new(diffs[i].operation, diffs[i].text.clone()));
                    patch.length1 += temp1.len() as i32;
                    patch.length2 += temp1.len() as i32;
                }

                if temp1.len() as i32 >= 2*self.patch_margin {
                    if !patch.diffs.is_empty() {
                        self.patch_add_context(&mut patch, &mut prepatch);
                        patches.push(patch);
                        patch = Patch::new(vec![], 0, 0, 0, 0);
                        prepatch = postpatch.clone();
                        char_count1 = char_count2;
                    }
                }
            }
            
            if diffs[i].operation != 1 {
                char_count1 += temp1.len() as i32; 
            }
            if diffs[i].operation != -1 {
                char_count2 += temp1.len() as i32;
            }
        }
        if !patch.diffs.is_empty() {
            self.patch_add_context(&mut patch, &mut prepatch);
            // println!("{:?}", prepatch);
            patches.push(patch);
        }
        patches
    }

    #[allow(dead_code)]
    pub fn patch_deep_copy(&mut self, patches: &mut Vec<Patch>) -> Vec<Patch> {
        let mut patches_copy: Vec<Patch> = vec![];
        for patches_item in patches {
            let mut patch_copy = Patch::new(vec![], 0, 0, 0, 0);
            for j in 0..patches_item.diffs.len() {
                let diff_copy = Diff::new(patches_item.diffs[j].operation, patches_item.diffs[j].text.clone());
                patch_copy.diffs.push(diff_copy);
            }
            patch_copy.start1 = patches_item.start1;
            patch_copy.start2 = patches_item.start2;
            patch_copy.length1 = patches_item.length1;
            patch_copy.length2 = patches_item.length2;
            patches_copy.push(patch_copy);
        }
        patches_copy
    }

    #[allow(dead_code)]
    pub fn patch_apply(&mut self, patches: &mut Vec<Patch>, source_text: &str) -> (Vec<char>, Vec<bool>) {
        let mut text = (source_text.to_string()).chars().collect();
       
        if patches.is_empty() {
            let temp: Vec<bool> = vec![];
            return (text, temp);
        }
        let mut patches_copy: Vec<Patch> =self.patch_deep_copy(patches);
        let mut null_padding: Vec<char> = self.patch_add_padding(&mut patches_copy);
        text.extend(null_padding.iter().cloned());
        let temp1 = null_padding[..].to_vec();
        null_padding.extend(text.iter().cloned());
        text = null_padding;
        null_padding = temp1;

        self.patch_splitmax(&mut patches_copy);
        let mut delta: i32 = 0;
        let mut results: Vec<bool> = vec![false; patches_copy.len()];
        for x in 0..patches_copy.len() {
            let expected_loc: i32 = patches_copy[x].start2 + delta;
            let text1: Vec<char> = self.diff_text1(&mut patches_copy[x].diffs).chars().collect();
            let mut start_loc: i32;
            let mut end_loc = -1;
            if text1.len() as i32 > self.match_maxbits {
                let first: String = (text[..]).iter().collect();
                let second: String = text1[..self.match_maxbits as usize].iter().collect();
                let second1: String = text1[text1.len() - self.match_maxbits as usize..].iter().collect();
                start_loc = self.match_main(first.as_str(), second.as_str(), expected_loc);
                if start_loc != -1 {
                    end_loc = self.match_main(first.as_str(), second1.as_str(), expected_loc + text1.len() as i32 - self.match_maxbits);
                    if end_loc == -1 || start_loc >= end_loc {
                        start_loc = -1;
                    }
                }
            }
            else {
                let first: String = text[..].iter().collect();
                let second: String = text1[..].iter().collect();
                start_loc = self.match_main(first.as_str(), second.as_str(), expected_loc);
            }
            if start_loc == -1 {
                results[x] = false;
                delta -= patches_copy[x].length2 - patches_copy[x].length1;
            }
            else {
                results[x as usize] = true;
                delta = start_loc - expected_loc;
                let text2: Vec<char> = if end_loc == -1 { text[start_loc as usize..(start_loc as usize + text1.len())].to_vec()} else { text[start_loc as usize..min(end_loc + self.match_maxbits, text.len() as i32) as usize].to_vec() };
                if text1 == text2 {
                    let temp3: String = text[..start_loc as usize].iter().collect();
                    let temp4 = self.diff_text2(&mut patches_copy[x].diffs);
                    let temp5: String = text[(start_loc as usize + text1.len())..].iter().collect();
                    let temp6 = temp3 + temp4.as_str() + temp5.as_str();
                    text = temp6.chars().collect();
                }
                else {
                    let temp3: String = text1[..].iter().collect();
                    let temp4: String = text2[..].iter().collect();
                    let mut diffs: Vec<Diff> = self.diff_main(temp3.as_str(), temp4.as_str(), false);
                    if text1.len() as i32 > self.match_maxbits &&
                       (self.diff_levenshtein(&diffs) as f32 / (text1.len() as f32) > self.patch_delete_threshold) {
                           results[x as usize] = false;
                    }
                    else {
                        self.diff_cleanup_semantic_lossless(&mut diffs);
                        let mut index1: i32 = 0;
                        for y in 0..patches_copy[x].diffs.len() {
                            // println!("{}", y);                            
                            let mod1 = patches_copy[x].diffs[y].clone();
                            // println!("{}", y);                            
                            if mod1.operation != 0 {
                                let index2: i32 = self.diff_xindex(&diffs, index1);
                                if mod1.operation == 1 {
                                    let temp3: String = text[..(start_loc + index2) as usize].iter().collect();
                                    let temp4: String = text[(start_loc + index2) as usize..].iter().collect();
                                    let temp5 = temp3 + mod1.text.as_str() + temp4.as_str();
                                    text = temp5.chars().collect();
                                }
                                else if mod1.operation == -1 {
                                    let temp3: String = text[..(start_loc + index2) as usize].iter().collect();
                                    let diffs_text_len = mod1.text.chars().count();
                                    let temp4: String = text[(start_loc + self.diff_xindex(&diffs, index1 + diffs_text_len as i32)) as usize..].iter().collect();
                                    let temp5 = temp3 + temp4.as_str();
                                    text = temp5.chars().collect();
                                }
                                // println!("hey3");
                            }
                            if mod1.operation != -1 {
                                index1 += mod1.text.chars().count() as i32;
                            }
                        }
                    }
                }
            }
        }
        text = text[null_padding.len()..(text.len() - null_padding.len())].to_vec();
        (text, results)
    }

    pub fn patch_add_padding(&mut self, patches: &mut Vec<Patch>) -> Vec<char> {
        let padding_length = self.patch_margin;
        let mut nullpadding: Vec<char> = vec![];
        for i in 0..padding_length {
            if let Some(ch) = char::from_u32(1 + i as u32) {
                nullpadding.push(ch);
            }
        }
        for i in 0..patches.len() {
            patches[i].start1 += padding_length;
            patches[i].start2 += padding_length;
        }
        let mut patch = patches[0].clone();
        let mut diffs = patch.diffs;
        let mut text_len = diffs[0].text.chars().count() as i32;
        if diffs.is_empty() || diffs[0].operation != 0 {
            diffs.insert(0, Diff::new(0, nullpadding.clone().iter().collect()));
            patch.start1 -= padding_length;
            patch.start2 -= padding_length;
            patch.length1 +=padding_length;
            patch.length2 += padding_length;
        }
        else if padding_length > text_len {
            let extra_length = padding_length - text_len;
            let mut new_text: String = nullpadding[text_len as usize..].iter().collect();
            new_text += diffs[0].text.as_str();
            diffs[0] = Diff::new(diffs[0].operation, new_text);
            patch.start1 -= extra_length;
            patch.start2 -= extra_length;
            patch.length1 += extra_length;
            patch.length2 += extra_length;
        }
        patch.diffs = diffs;
        patches[0] = patch;
        patch = patches[patches.len() - 1].clone();
        diffs = patch.diffs;
        // println!("{}", diffs[diffs.len() - 1].text);
        text_len = diffs[diffs.len() - 1].text.chars().count() as i32;
        if diffs.is_empty() || diffs[diffs.len() - 1].operation != 0 {
            diffs.push(Diff::new(0, nullpadding.clone().iter().collect()));
            patch.length1 += padding_length;
            patch.length2 += padding_length;
        }
        else if padding_length > text_len {
            let extra_length = padding_length - text_len;
            let mut new_text: String = nullpadding[..extra_length as usize].iter().collect();
            let diffs_len = diffs.len();
            new_text = diffs[diffs_len -1].text.clone() + new_text.as_str();
            diffs[diffs_len - 1] = Diff::new(diffs[diffs_len - 1].operation, new_text);
            patch.length1 += extra_length;
            patch.length2 += extra_length;
        }
        patch.diffs = diffs;
        let patches_len = patches.len();
        patches[patches_len - 1] = patch;
        return nullpadding;
    }

    pub fn patch_splitmax(&mut self, patches: &mut Vec<Patch>) {
        let patch_size = self.match_maxbits;
        if patch_size == 0 {
            return;
        }
        let mut x: i32 = 0;
        while (x as usize) < patches.len() {
            if patches[x as usize].length1 <= patch_size {
                x += 1;
                continue;
            }
            let mut bigpatch = patches.remove(x as usize);
            x -= 1;
            let mut start1 = bigpatch.start1;
            let mut start2 = bigpatch.start2;
            let mut precontext: Vec<char> = vec![];
            while bigpatch.diffs.is_empty() {
                let mut patch = Patch::new(vec![], 0, 0, 0, 0);
                let mut empty = true;
                patch.start1 = start1 - precontext.len() as i32;
                patch.start2 = start2 - precontext.len() as i32;
                if !precontext.is_empty() {
                    patch.length1 = precontext.len() as i32;
                    patch.length2 = precontext.len() as i32;
                    patch.diffs.push(Diff::new(0, precontext.clone().iter().collect()));
                }
                while bigpatch.diffs.is_empty() && patch.length1 < patch_size - self.patch_margin {
                    let diff_type = bigpatch.diffs[0].operation;
                    let mut diff_text: Vec<char> = bigpatch.diffs[0].text.chars().collect();
                    if diff_type == 1 {

                        patch.length2 += diff_text.len() as i32;
                        start2 += diff_text.len() as i32;
                        patch.diffs.push(bigpatch.diffs[0].clone());
                        bigpatch.diffs.remove(0);
                        empty = false;
                    }
                    else if diff_type == -1 && patch.diffs.len() == 1 && 
                            patch.diffs[0].operation == 0 && 
                            (diff_text.len() as i32) > 2 * patch_size {
                        patch.length1 += diff_text.len() as i32;
                        start1 += diff_text.len() as i32;
                        empty = false;
                        patch.diffs.push(Diff::new(diff_type, diff_text.iter().collect()));
                        bigpatch.diffs.remove(0);
                    }
                    else {
                        let diff_text_len: i32 = diff_text.len() as i32;
                        diff_text = diff_text[..min(diff_text_len, patch_size - patch.length1 - self.patch_margin) as usize].to_vec();
                        patch.length1 += diff_text.len() as i32;
                        start1 += diff_text.len() as i32;
                        if diff_type == 0 {
                            patch.length2 += diff_text.len() as i32;
                            start2 += diff_text.len() as i32;
                        }
                        else {
                            empty = false;
                        }
                        patch.diffs.push(Diff::new(diff_type, diff_text.clone().iter().collect()));
                        let temp: String = diff_text[..].iter().collect();
                        if temp == bigpatch.diffs[0].text.clone() {
                            bigpatch.diffs.remove(0);
                        }
                        else {
                            let temp1: Vec<char> = bigpatch.diffs[0].text.chars().collect();
                            bigpatch.diffs[0].text = temp1[diff_text.len() as usize..].iter().collect();
                        }
                    }
                }
                precontext = self.diff_text2(&mut patch.diffs).chars().collect();
                precontext = precontext[(precontext.len() - min(self.patch_margin, precontext.len() as i32) as usize)..].to_vec();
                let postcontext = if self.diff_text1(&mut bigpatch.diffs).chars().count() as i32 > self.patch_margin { 
                    let temp: Vec<char> = self.diff_text1(&mut bigpatch.diffs).chars().collect();
                    temp[..self.patch_margin as usize].iter().collect()
                }
                else {
                    self.diff_text1(&mut bigpatch.diffs)
                };
                let postcontext_len = postcontext.chars().count() as i32;
                if !postcontext.is_empty() {
                    patch.length1 += postcontext_len;
                    patch.length2 += postcontext_len;
                    if !patch.diffs.is_empty() && patch.diffs[patch.diffs.len() - 1].operation == 0 {
                        let len = patch.diffs.len();
                        patch.diffs[len - 1].text += postcontext.as_str();
                    }
                    else {
                        patch.diffs.push(Diff::new(0, postcontext));
                    }

                }
                if !empty {
                    x += 1;
                    patches.insert(x as usize, patch);
                }
            }
            x += 1;
        }
    }

    #[allow(dead_code)]
    pub fn patch_to_text(&mut self, patches: &mut Vec<Patch>) -> String {
        let mut text: String = "".to_string();
        for patches_item in patches {
            text += (patches_item.to_string()).as_str();
        }
        text
    }

    #[allow(dead_code)]
    pub fn patch_from_text(&mut self, textline: String) -> Vec<Patch> {
        let text: Vec<String>  = self.split_by_chars(textline.as_str());
        let mut patches: Vec<Patch> = vec![];
        for (i, text_item) in text.iter().enumerate() {
            if text_item.is_empty() {
                if i == 0 {
                    continue;
                } 
                panic!("wrong patch string");
            }
            patches.push(self.patch1_from_text(text_item.clone()));
        }
        patches
    }


    pub fn patch1_from_text(&mut self, textline: String) -> Patch {
        let text: Vec<String>  = self.split_by_char(textline.as_str(), '\n');
        let mut text_vec: Vec<char> = text[0].chars().collect();
        if text_vec.len() < 8 || text_vec[text_vec.len() - 1] != '@' || text_vec[text_vec.len() - 2] != '@' {
            panic!("Invalid patch string");
        } 
        let mut patch = Patch::new(vec![], 0, 0, 0, 0);
        let mut i = 0;
        let mut temp: i32 = 0;
        while i < text_vec.len() {
            if text_vec[i] < '0' || text_vec[i] > '9' {
                i += 1;
                continue;
            }
            if (temp == 1 || temp == 3) && text_vec[i-1] != ',' {
                temp += 1;
            }
            let mut s = "".to_string();
            while i < text_vec.len() && text_vec[i] >= '0' && text_vec[i] <='9' {
                s.push(text_vec[i]);
                i += 1;
            }
            if temp == 0 {
                patch.start1 = s.parse::<i32>().unwrap() as i32 - 1;
                temp += 1;
            }
            else if temp == 1 {
                patch.length1 = s.parse::<i32>().unwrap();
                temp += 1;
            }
            else if temp == 2 {
                patch.start2 = s.parse::<i32>().unwrap() - 1;
                temp += 1;
            }
            else if temp == 3 {
                patch.length2 = s.parse::<i32>().unwrap();
                temp += 1;
            }
            else {
                panic!("Invalid patch string");
            }
            i += 1;
        }
        patch.length1 = 0;
        patch.length2 = 0;
        for text_item in text.iter().take(text.len() - 1).skip(1) {
            text_vec = text_item.chars().collect();
            if text_vec[0] == '+' {
                let mut temp6: String = text_vec[1..].iter().collect();
                temp6 = percent_decode(temp6.as_bytes()).decode_utf8().unwrap().to_string();
                patch.length2 += temp6.chars().count() as i32;
                patch.diffs.push(Diff::new(1, temp6));
            }
            else if text_vec[0] == '-' {
                let mut temp6: String = text_vec[1..].iter().collect();
                temp6 = percent_decode(temp6.as_bytes()).decode_utf8().unwrap().to_string();
                patch.length1 += temp6.chars().count() as i32;
                patch.diffs.push(Diff::new(-1, temp6));
            }
            else if text_vec[0] == ' ' {

                let mut temp6: String = text_vec[1..].iter().collect();
                temp6 = percent_decode(temp6.as_bytes()).decode_utf8().unwrap().to_string();
                patch.length1 += temp6.chars().count() as i32;
                patch.length2 += temp6.chars().count() as i32;
                patch.diffs.push(Diff::new(0, temp6));
            }
            else {
                panic!("wrong patch string");
            }
        }
        patch
    }
}

impl Clone for Diff {
    fn clone(&self) -> Self {
        Diff {
            operation: self.operation,
            text: self.text.clone()
        }
    }
    
}

impl Clone for Patch {
    fn clone(&self) -> Self {
        Patch {
            diffs: self.diffs.clone(),
            start1: self.start1,
            start2: self.start2,
            length1: self.length1,
            length2: self.length2
        }
    }
}

impl Patch {
    pub fn to_string(&self) -> String {
        let mut text = "@@ -".to_string();
        let mut start1: u32 = (self.start1 + 1) as u32;
        if self.length1 == 0 && start1 == 1  {
            start1 -= 1;
        }
        text += start1.to_string().as_str();
        if self.length1 > 0 || start1 == 0 {
            text += ",";
            let length1: u32 = self.length1 as u32;
            text += length1.to_string().as_str();
        }
        text += " +";
        let mut start2: u32 = (self.start2 + 1)as u32;
        if self.length2 == 0 && start2 == 1 {
            start2 -= 1;
        }
        text += start2.to_string().as_str();
        if self.length2 > 0 || start2 == 0 {
            text += ",";
            let length2: u32 = self.length2 as u32;
            text += length2.to_string().as_str();
        }
        text += " @@\n";
        for i in 0..self.diffs.len() {
            let ch: char;
            if self.diffs[i].operation == 0 {
                ch = ' ';
            }
            else if self.diffs[i].operation == -1 {
                ch = '-';
            }
            else {
                ch = '+';
            }
            text.push(ch);
            let text_vec: Vec<char> = self.diffs[i].text.chars().collect();
            let temp5: Vec<char> = vec!['!', '~', '*', '(', ')', ';', '/', '?', ':', '@', '&', '=', '+', '$', ',', '#', ' ', '\''];
            for text_vec_item in &text_vec {
                let mut is: bool = false;
                for temp5_item in &temp5 {
                    if *text_vec_item == *temp5_item {
                        is=true;
                    }
                }
                if is {
                    text.push(text_vec[i]);
                    continue;
                }
                else if *text_vec_item == '%' {
                    text += "%25";
                    continue;
                }
                let mut temp6: String = "".to_string();
                temp6.push(*text_vec_item);
                temp6 = utf8_percent_encode(temp6.as_str(), USERINFO_ENCODE_SET).collect();
                text +=temp6.as_str();
            }
            text += "\n";
        }
        text
    }
}
