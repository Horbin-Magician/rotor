

struct File {
    parent_index: u64,
    file_name: String,
    filter: u32,
    rank: u8,
}

struct SearchResultItem {
    path: String,
    file_name: String,
    rank: u8,
}

struct SearchResult {
    items: Vec<SearchResultItem>,
    query: String,
}

type FileMap = std::collections::HashMap<u64, File>;

struct Volume {
    state: uint,
    drive: char,
    drive_FRN: u64,
    file_map: FileMap,
    stop_find: bool,
    start_USN: u64,
    UJD: u64,
    h_vol: u64,
}

impl Volume {
    pub fn new(drive: char) {
        // let h_vol = Open(drive, GENERIC_READ); // TODO
        Volume {
            state: 0,
            drive,
            drive_FRN: 0x5000000000005,
            file_map: FileMap::new(),
            stop_find: false,
            start_USN: 0x0,
            UJD: 0x0,
            // h_vol // TODO
        }
    }

    // searching
    pub fn Fine(query: string) -> vec<SearchResultItem> {
        vec![] // TODO
    // vector<SearchResultFile>* Volume::Find(QString strQuery){
    //     // init timer
    //     QElapsedTimer timer;
    //     timer.start();

    //     if(strQuery.length() == 0) return nullptr;
    //     if(m_FileMap.isEmpty()) SerializationRead();

    //     QString strQuery_lower = strQuery.toLower();

    //     vector<SearchResultFile>* rgsrfResults = new vector<SearchResultFile>();

    //     DWORD queryFilter = MakeFilter(strQuery_lower); //Calculate Filter value which are compared with the cached ones to skip many of them

    //     m_FileMapMutex.tryLock(1);
    //     for(QMap<DWORDLONG, File*>::iterator it = m_FileMap.begin(); it != m_FileMap.end(); ++it){
    //         if(m_StopFind){
    //             m_StopFind = false;
    //             delete rgsrfResults;
    //             return nullptr;
    //         }

    //         if((it.value()->filter & queryFilter) == queryFilter){
    //             QString sz = it.value()->getStrName();
    //             char rank = MatchStr(sz, strQuery_lower);
    //             if(rank >= 0){
    //                 SearchResultFile srf;
    //                 srf.path.reserve(MAX_PATH);
    //                 if(GetPath(it.value()->parentIndex, &srf.path)){
    //                     srf.filename = sz;
    //                     srf.rank = it.value()->rank + rank;
    //                     rgsrfResults->insert(rgsrfResults->end(), srf);
    //                 }
    //             }
    //         }
    //     }
    //     m_FileMapMutex.unlock();

    //     qDebug() << (char)m_drive  << "[计时信息] Find用时：" << timer.elapsed() << "milliseconds";

    //     return rgsrfResults;
    // }
    }

    // Enumerate the MFT for all entries. Store the file reference numbers of any directories in the database.
    pub fn BuildIndex() {
        // TODO
    // void Volume::BuildIndex(){
    //     // init timer
    //     QElapsedTimer timer;
    //     timer.start();

    //     m_FileMapMutex.lock();

    //     ReleaseIndex(false);

    //     Query(&m_ujd);
    //     m_StartUSN = m_ujd.NextUsn;

    //     // add the root directory
    //     WCHAR szRoot[_MAX_PATH];
    //     wsprintf(szRoot, TEXT("%c:"), m_drive);
    //     AddFile(m_driveFRN, szRoot, 0);

    //     MFT_ENUM_DATA med;
    //     med.StartFileReferenceNumber = 0;
    //     med.LowUsn = 0;
    //     med.HighUsn = m_ujd.NextUsn;

    //     BYTE pData[sizeof(DWORDLONG) * 0x10000];
    //     DWORD cb;

    //     while (DeviceIoControl(m_hVol, FSCTL_ENUM_USN_DATA, &med, sizeof(med), pData, sizeof(pData), &cb, NULL)){
    //         PUSN_RECORD pRecord = (PUSN_RECORD) &pData[sizeof(USN)];
    //         while ((PBYTE) pRecord < (pData + cb)){
    //             wstring sz((LPCWSTR) ((PBYTE) pRecord + pRecord->FileNameOffset), pRecord->FileNameLength / sizeof(WCHAR));
    //             AddFile(pRecord->FileReferenceNumber, sz, pRecord->ParentFileReferenceNumber);
    //             pRecord = (PUSN_RECORD) ((PBYTE) pRecord + pRecord->RecordLength);
    //         }
    //         med.StartFileReferenceNumber = * (USN *) pData;
    //     }

    //     SerializationWrite();
    //     m_FileMapMutex.unlock();

    //     qDebug() << (char)m_drive  << "[计时信息] BuildIndex用时：" << timer.elapsed() << "milliseconds";
    // }
    }

    pub fn UpdateIndex() {
        // TODO
    // void Volume::UpdateIndex(){
    //     if(m_FileMap.isEmpty()) SerializationRead();
    
    //     WCHAR szRoot[_MAX_PATH];
    //     wsprintf(szRoot, TEXT("%c:"), m_drive);
    
    //     BYTE pData[sizeof(DWORDLONG) * 0x10000];
    //     DWORD cb;
    //     DWORD reason_mask = USN_REASON_FILE_CREATE | USN_REASON_FILE_DELETE | USN_REASON_RENAME_NEW_NAME;
    //     READ_USN_JOURNAL_DATA rujd = {m_StartUSN, reason_mask, 0, 0, 0, m_ujd.UsnJournalID};
    
    //     m_FileMapMutex.lock();
    //     while (DeviceIoControl(m_hVol, FSCTL_READ_USN_JOURNAL, &rujd, sizeof(rujd), pData, sizeof(pData), &cb, NULL)){
    //         if(cb == 8) break;
    //         PUSN_RECORD pRecord = (PUSN_RECORD) &pData[sizeof(USN)];
    //         while ((PBYTE) pRecord < (pData + cb)){
    //             wstring sz((LPCWSTR) ((PBYTE) pRecord + pRecord->FileNameOffset), pRecord->FileNameLength / sizeof(WCHAR));
    //             if ((pRecord->Reason & USN_REASON_FILE_CREATE) == USN_REASON_FILE_CREATE){
    //                 AddFile(pRecord->FileReferenceNumber, sz, pRecord->ParentFileReferenceNumber);
    //             }
    //             else if ((pRecord->Reason & USN_REASON_FILE_DELETE) == USN_REASON_FILE_DELETE){
    //                 m_FileMap.remove(pRecord->FileReferenceNumber);
    //             }
    //             else if ((pRecord->Reason & USN_REASON_RENAME_NEW_NAME) == USN_REASON_RENAME_NEW_NAME){
    //                 AddFile(pRecord->FileReferenceNumber, sz, pRecord->ParentFileReferenceNumber);
    //             }
    //             pRecord = (PUSN_RECORD) ((PBYTE) pRecord + pRecord->RecordLength);
    //         }
    //         rujd.StartUsn = *(USN *)&pData;
    //     }
    //     m_FileMapMutex.unlock();
    
    //     m_StartUSN = rujd.StartUsn;
    // }
    }

    // Clears the database
    pub fn ReleaseIndex(ifLock: bool) {
        // TODO
    // void Volume::ReleaseIndex(bool ifLock)
    // {
    
    //     if(ifLock) m_FileMapMutex.lock();
    //     qDeleteAll(m_FileMap);
    //     m_FileMap.clear();
    //     if(ifLock) m_FileMapMutex.unlock();
    // }
    }

    pub fn StopFind() {
        // TODO
    // void Volume::StopFind(){
    //     m_StopFind = true;
    // }
    }

    fn SerializationWrite() {
        // TODO
    // void Volume::SerializationWrite()
    // {
    //     if(m_FileMap.isEmpty()) return;

    //     QString appPath = QApplication::applicationDirPath(); // get programe path
    //     QFile file(appPath + "/userdata/" + m_drive + ".fd");
    //     file.open(QIODevice::WriteOnly | QIODevice::Truncate);
    //     QDataStream out(&file);

    //     out<<m_StartUSN;

    //     QMapIterator<DWORDLONG, File*> i(m_FileMap);
    //     while (i.hasNext()){
    //         i.next();
    //         const File* filedata = i.value();
    //         out<<i.key()<<filedata->parentIndex<<filedata->fileName<<(quint32)filedata->filter<<filedata->rank;
    //     }

    //     this->ReleaseIndex(false);
    //     file.close();
    // }
    }

    fn SerializationRead() {
        // TODO
    // void Volume::SerializationRead()
    // {
    //     QString appPath = QApplication::applicationDirPath(); // get programe path
    //     QFile file(appPath + "/userdata/" + m_drive + ".fd");
    //     file.open(QIODevice::ReadOnly);
    //     QDataStream in(&file);

    //     DWORDLONG index;
    //     DWORDLONG parentIndex;
    //     QByteArray fileName;
    //     quint32 filter;
    //     char rank;

    //     in>>m_StartUSN;

    //     m_FileMapMutex.lock();
    //     while(in.atEnd() == false){
    //         in>>index>>parentIndex>>fileName>>filter>>rank;
    //         m_FileMap[index] = new File(parentIndex, fileName, filter, rank);
    //     }
    //     m_FileMapMutex.unlock();

    //     file.close();
    // }
    }

    // This is a helper function that opens a handle to the volume specified by the cDriveLetter parameter.
    fn Open(c_drive_letter: char, dw_access: u32) -> u64 {
        // TODO
        0
    // HANDLE Volume::Open(TCHAR cDriveLetter, DWORD dwAccess){
    //     TCHAR szVolumePath[_MAX_PATH];
    //     wsprintf(szVolumePath, TEXT("\\\\.\\%c:"), cDriveLetter);
    //     HANDLE hCJ = CreateFile(szVolumePath, dwAccess, FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, OPEN_EXISTING, 0, NULL);
    //     return(hCJ);
    // }
    }

    // Return statistics about the journal on the current volume
    fn Query(p_usn_journal_data: u64) -> bool {
        // TODO
        false
    // bool Volume::Query(PUSN_JOURNAL_DATA pUsnJournalData){
    //     DWORD cb;
    //     BOOL fOk = DeviceIoControl(m_hVol, FSCTL_QUERY_USN_JOURNAL, NULL, 0, pUsnJournalData, sizeof(*pUsnJournalData), &cb, NULL);
    //     return(fOk);
    // }
    }

    // Calculates a 32bit value that is used to filter out many files before comparing their filenames
    fn MakeFilter(str: string) -> u32 {
        // TODO
        0
    // DWORD Volume::MakeFilter(const QString& str)
    // {
    //     /*
    //     Creates an address that is used to filter out strings that don't contain the queried characters
    //     Explanation of the meaning of the single bits:
    //     0-25 a-z
    //     26 0-9
    //     27 other ASCII
    //     28 not in ASCII
    //     */
    //     uint len = str.length();
    //     if(len <= 0) return 0;
    //     uint32_t Address = 0;

    //     QString szlower = str.toLower();

    //     char c;

    //     for(uint i = 0; i != len; ++i)
    //     {
    //         c = szlower[i].toLatin1();
    //         if(c > 96 && c < 123) Address |= (uint32_t)1 << ((uint32_t)c - (uint32_t)97); //a-z
    //         else if(c >= L'0' && c <= '9') Address |= (uint32_t)1 << 26; //0-9
    //         else if(c == 0) Address |= (uint32_t)1 << 28; // not in ASCII
    //         else Address |= (uint32_t)1 << 27; // other ASCII
    //     }
    //     return Address;
    // }
    }

    // Adds a file to the database
    fn AddFile(index: u64, file_name: string, parent_index: u64) -> bool {
        // TODO
        false
    // bool Volume::AddFile(DWORDLONG index, wstring fileName, DWORDLONG parentIndex){
    //     QString qFileName = QString::fromStdWString(fileName);
    //     DWORD filter = MakeFilter(qFileName);
    //     char rank = GetFileRank(qFileName);
    //     m_FileMap[index] = new File(parentIndex, qFileName, filter, rank);
    //     return(TRUE);
    // }

    // char Volume::MatchStr(const QString &contain, const QString &query_lower)
    // {
    //     int i = 0;
    //     foreach (QChar c, contain.toLower()) {
    //         if(query_lower[i] == c) ++i;
    //         if(i >= query_lower.length()){
    //             int rank = 10 - (contain.length() - query_lower.length());
    //             return rank < 0 ? 0 : rank;
    //         }
    //     }
    //     return -1;
    // }
    }

    // Constructs a path for a directory
    fn GetPath(index: u64, sz: string) -> bool {
        // TODO
        false
    // bool Volume::GetPath(DWORDLONG index, QString *sz)
    // {
    //     *sz = "";
    //     while (index != 0){
    //         if(m_FileMap.contains(index) == false) return false;
    //         File* file = m_FileMap[index];
    //         *sz = file->getStrName() + "\\" + *sz;
    //         index = file->parentIndex;
    //     };
    //     return TRUE;
    // }
    }

    fn MatchStr(contain: string, query: string) -> char {
        // TODO
        0
    }

    // return rank by filename
    fn GetFileRank(file_name: string) -> u8 {
        // TODO
        0
    // char Volume::GetFileRank(const QString& fileName)
    // {
    //     char rank = 0;
    //     if(fileName.endsWith(L".exe", Qt::CaseInsensitive))rank += 10;
    //     else if(fileName.endsWith(L".lnk", Qt::CaseInsensitive)) rank += 30;
    //     return rank;
    // }
    }
}